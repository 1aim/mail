// a module level circ. dep. but fine as only
// used for more ergonomic helper constructors
use context::Context;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

// We can provide a resource through two ways
// 1. providing a Source (iri + media_type_override + name override)
// 2. providing the actual data
// 3. we could provide transfer encoded resources, normally we do not

// We normally use a resource only in a transfer encoded way.
//
// - So if we get a source we have to load the actual data.
// - If we get the actual data we still have to transfer encode it.
//

// We normally do not need to keep the actual data once we did the transfer
// encoding.
//
// BUT the best chosen encoding might depend on the actual case mainly
// the mail type e.g. if the mail type is Mime8Bit or Internationalized
// and our data is utf-8 we might want to send it _without_ encoding (but
// this means we need to check for boundaries).
//
// BUT we also want to cache the encoded data, sometimes more than the
// actual data.
//
// Also we normally want to encode text (any ascii compatible encoding)
// with quoted printable but BUT this is not the case for e.g. chinese
// text in utf-8 which would be horrible to encode with Quoted Printable
// (either no encoding or base64)

/*
      Mail  CtxUtils            Ctx Impl
Source->|    |                     |
        |    | load_data(&Source)  |
        |----+-------------------->|
       XOR   | encode_data(&Data)  |
        |----+-------------------->|
        |    |                     |
        |    |  encode_data(&Data) |
        |    |<-- Data ------------|  uses IRI to cache
        |    |(contain content id) |  return with CID
        |    |                     |  (for supporting caching, data needs to be Arc'ed)
        |    |                     |
        |    |                     |  uses the CID to cache, we can't use
        |    |                     |  the data BUT that means it must be
        |    |---EncData---------->|  immutable
        |<---+-- EncData ----------|  (for supporting caching, enc data needs to be Arc'ed)
        |    |                     |
        |    |                     |  The encoding hint (`EncHint`) can change
        |    |                     |  sometimes this means re-encoding, sometimes
        |    |                     |  this means encoding what didn't need to be
        |                          |  encoded, sometimes this means encoding, sometimes
        |                          |  we can just get the cached data.
        |                          |
        |                          |  The encoding hint should also contain a batch of
        |                          |  boundaries which can be checked "on the fly" while
        |                          |  encoding (or checking for validity).
        |                          |
        |                          |  Most server support Mime8Bit and most text bodies
        |                          |  will be utf-8 so normally we could use represent them
        |                          |  "as is" expect that there is still a line length limit
        |                          |  and potential "wrong" line breaks.
        |                          |  But most mails should not break the hard line length
        |                          |  limit and normally "wrong" line breaks can be "fixed"
        |                          |  on the fly.

    How does this integrate with into encodable mail?

    - will replace all occurrences where Resources are
      Source or Data with encoded Data while also setting
      boundaries, content-transfer-encoded headers

    The last question left open is:
        - how to handle the case where a mail are encoded assuming mime 8bit
          but then it's not supported? What is affected?
          - the data needs to be encoded
          - boundaries do not need to be changed (we generate boundaries
            so that they cant conflict with quoted printable or base64
            encoded data)
          - Content Transfer Encoding header needs to be changed
            - Content-Transfer-Encoding headers are kinda invisible
              you must not add them to header maps, we auto add them
              when turning it into an encodable mail and auto remove
              it when turning it back into a mail

    So what can we do?
        - Instead of adding Content-Transfer-Encoding header just
          "on the fly" encode a non added header here (so we can
          also "on the fly" encode a different header).
        - We can also "on the fly" encode non encoded data and write
          the encoded data instead of the normal data.

    So?
        - encoding non Mime8Bit will "on the fly" take longer,
          but it still _should be_ affordable longer and not cause
          anything like a timeout


  .get_boundary_pool()

  .load_data(&Source, &mut BoundaryPool) -> Result
  .encode_data(&Data, &mut BoundaryPool) -> Result
    (both remove colliding boundaries)

  data.transfer_encode(EncHint, &mut BoundaryPool) -> Result<EncData>

  EncHint:
    - don't assume Mime8Bit
    - Use Base64
    - Use QuotedPrintable
    - NotHint
*/
use headers::header_components::ContentId;

mod data;
mod loading;
mod source;

pub use self::data::*;
pub use self::loading::*;
pub use self::source::*;

/// A enum specifying a "resource" for a mail.
///
/// A resource represents any kind of actual data.
/// It can be anything from a html body of a mail over a embedded
/// image to a attached spread sheet.
///
/// A resource can be specified in 3 ways:
/// 1. As a source specifying what to get and how to handle it.
/// 2. Data (and Metadata) representing a resource.
/// 3. Data (and Metadata) representing a transfer encoded resource.
///
/// Normally auto generated content will be provided as `Data`, embeddings
/// and attachments will be provided as `Source` (potentially referring to
/// a file on in a file system) and transfer encoded data can not be provided
/// by the user.
///
/// When a mail is converted to a encodable mail any resource will be swapped
/// with a version of it which is transfer encoded, so the only way a consumer
/// of this library normally comes in contact with the third variant is by
/// turning a encodable mail back into normal mail.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum Resource {
    /// Provide a source which specify what data to use.
    ///
    /// This also allows specifying a media type (if not stored with the data
    /// and sniffing is not wanted or to unreliable).
    ///
    /// Additionally it allows to specify a "file name" which will force the
    /// given name to be used instead of inferring it from the IRI or meta data
    /// associated with the IRI in "some way" (like file name fild in a database
    /// from which the data will loaded).
    Source(Source),

    /// Provide the data used for the mail bodies content.
    ///
    /// This for example could be a png image.
    Data(Data),

    /// Provides a already transfer encoded version of the `Data` variant.
    ///
    /// This can not be created by a consumer of the library and will be
    /// created when turning a mail into a transfer encoded mail.
    EncData(EncData),
}

impl Resource {
    /// Creates a new text `Resource` with `text/plain; charset=utf-8` media type.
    ///
    /// The `Context` is used to generate a `ContentId`.
    pub fn plain_text(content: impl Into<String>, ctx: &impl Context) -> Resource {
        Resource::Data(Data::plain_text(content, ctx.generate_content_id()))
    }

    /// Return the content id, if there is any.
    pub fn content_id(&self) -> Option<&ContentId> {
        match self {
            &Resource::Source(..) => None,
            &Resource::Data(ref data) => Some(data.content_id()),
            &Resource::EncData(ref enc_data) => Some(enc_data.content_id()),
        }
    }
}
