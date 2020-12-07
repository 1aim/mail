//! This module provides utilities for composing multipart mails.
//!
//! While the `Mail` type on itself can represent any multipart
//! mail most mails have a certain pattern to their structure,
//! consisting mainly of `multipart/mixed` for attachments,
//! `multipart/alternative` for alternative bodies and
//! `multipart/related` for including embedded resources which
//! can be used in the mail bodies  like e.g. a logo.
//!
//! This module provides the needed utilities to more simply
//! create a `Mail` instance which represents this kind of
//! mails.

//-------------------------------------------------------------\\
// NOTE: Implementations for creating (composing) mails are    ||
// split from the type dev, and normal impl blocks and placed  ||
// in the later part of the file for better readability.       ||
//-------------------------------------------------------------//

use media_type::{ALTERNATIVE, MIXED, MULTIPART, RELATED};
use vec1::Vec1;

use headers::{
    header_components::{Disposition, DispositionKind, MediaType},
    headers, HeaderKind,
};

use crate::{mail::Mail, resource::Resource};

/// Parts used to create a mail body (in a multipart mail).
///
/// This type contains a `Resource` which is normally used
/// to create a alternative body in a `multipart/alternative`
/// section. As well as a number of "embeddings" which depending
/// on there disposition can are either used as attachments
/// or embedded resource to which the body can referre to
/// by it's content id.
#[derive(Debug)]
pub struct BodyPart {
    /// A body created by a template.
    pub resource: Resource,

    /// A number of embeddings which should be displayed inline.
    ///
    /// This is normally used to embed images then displayed in
    /// a html body. It is not in the scope of this part of the
    /// library to bind content id's to resources to thinks using
    /// them to display the embeddings. This part of the library
    /// does "just" handle that they are correctly placed in the
    /// resulting Mail. The `mail-templates` crate provides more
    /// functionality for better ergonomics.
    ///
    /// Note that embeddings placed in a `BodyPart` instance are potentially
    /// only usable in the body specified in the same `BodyPart` instance.
    pub inline_embeddings: Vec<Resource>,

    /// A number of embeddings which should be treated as attachments.
    ///
    /// Attachments of a `BodyPart` instance will be combined with
    /// the attachments of other instances and the ones in the
    /// `MailParts` instance.
    pub attachments: Vec<Resource>,
}

/// Parts which can be used to compose a multipart mail.
///
/// This can be used to crate a mail, possible having
/// attachments with multiple alternative bodies having
/// embedded resources which can be referred to by the
/// bodies with content ids. This embeddings can be both
/// body specific or shared between bodies.
///
/// # Limitations
///
/// Any non alternative body will be either an attachment
/// or an body with a inline disposition header in a
/// `multipart/related` body. Which means you can not
/// use this mechanism to e.g. create a `multipart/mixed`
/// body with multiple disposition inline sub-bodies
/// which should be displayed side-by-side. Generally this
/// is not a very good way to create a mail, through a
/// valid way nevertheless.
///
pub struct MailParts {
    /// A vector of alternative bodies
    ///
    /// A typical setup would be to have two alternative bodies one text/html and
    /// another text/plain as fallback (for which the text/plain body would be
    /// the first in the vec and the text/html body the last one).
    ///
    /// Note that the order in the vector     /// a additional text/plainis
    /// the same as the order in which they will appear in the mail. I.e.
    /// the first one is the last fallback while the last one should be
    /// shown if possible.
    pub alternative_bodies: Vec1<BodyPart>,

    /// A number of embeddings which should be displayed inline.
    ///
    /// This is normally used to embed images then displayed in
    /// a html body. It is not in the scope of this part of the
    /// library to bind content id's to resources to thinks using
    /// them to display the embeddings. This part of the library
    /// does "just" handle that they are correctly placed in the
    /// resulting Mail. The `mail-templates` crate provides more
    /// functionality for better ergonomics.
    pub inline_embeddings: Vec<Resource>,

    /// A number of embeddings which should be treated as attachments
    pub attachments: Vec<Resource>,
}

//-------------------------------------------------------\\
//  implementations for creating mails are from here on  ||
//-------------------------------------------------------//

impl MailParts {
    /// Create a `Mail` instance based on this `MailParts` instance.
    ///
    ///
    /// If this instance contains any attachments then the
    /// returned mail will be a `multipart/mixed` mail with
    /// the first body containing the actual mail and the
    /// other bodies containing the attachments.
    ///
    /// If the `MailParts.inline_embeddings` is not empty then
    /// the mail will be wrapped in `multipart/related` (inside
    /// any potential `multipart/mixed`) containing the
    /// actual mail in the first body and the inline embeddings
    /// in the other bodies.
    ///
    /// The mail will have a `multipart/alternative` body
    /// if it has more then one alternative body
    /// (inside a potential `multipart/related` inside a
    /// potential `multipart/mixed` body). This body contains
    /// one sub-body for each `BodyPart` instance in
    /// `MailParts.alternative_bodies`.
    ///
    /// Each sub-body created for a `BodyPart` will be wrapped
    /// inside a `multipart/related` if it has body specific
    /// embeddings (with content disposition inline).
    pub fn compose(self) -> Mail {
        let MailParts {
            alternative_bodies,
            inline_embeddings,
            attachments,
        } = self;

        let mut attachments = attachments
            .into_iter()
            .map(|atta| atta.create_mail_with_disposition(DispositionKind::Attachment))
            .collect::<Vec<_>>();

        let mut alternatives = alternative_bodies
            .into_iter()
            .map(|body| body.create_mail(&mut attachments))
            .collect::<Vec<_>>();

        //UNWRAP_SAFE: bodies is Vec1, i.e. we have at last one
        let mail = alternatives.pop().unwrap();
        let mail = if alternatives.is_empty() {
            mail
        } else {
            mail.wrap_with_alternatives(alternatives)
        };

        let mail = if inline_embeddings.is_empty() {
            mail
        } else {
            let related = inline_embeddings
                .into_iter()
                .map(|embedding| embedding.create_mail_with_disposition(DispositionKind::Inline))
                .collect::<Vec<_>>();
            mail.wrap_with_related(related)
        };

        let mail = if attachments.is_empty() {
            mail
        } else {
            mail.wrap_with_mixed(attachments)
        };

        mail
    }
}

impl BodyPart {
    /// Creates a `Mail` instance from this `BodyPart` instance.
    ///
    /// All embeddings in `BodyPart.embeddings` which have a
    /// attachment content disposition are placed into the
    /// `attachments_out` parameter, as attachments should
    /// always be handled on the outer most level but the
    /// produced mail is likely not the outer most level.
    ///
    /// This will create a non-multipart body for the
    /// body `Resource`, if there are any embeddings which
    /// have a `Inline` disposition that body will be
    /// wrapped into a `multipart/related` body containing
    /// them.
    pub fn create_mail(self, attachments_out: &mut Vec<Mail>) -> Mail {
        let BodyPart {
            resource,
            inline_embeddings,
            attachments,
        } = self;

        let body = resource.create_mail();

        for attachment in attachments.into_iter() {
            let mail = attachment.create_mail_with_disposition(DispositionKind::Attachment);
            attachments_out.push(mail)
        }

        if inline_embeddings.is_empty() {
            body
        } else {
            let related = inline_embeddings
                .into_iter()
                .map(|embedding| embedding.create_mail_with_disposition(DispositionKind::Inline))
                .collect::<Vec<_>>();
            body.wrap_with_related(related)
        }
    }
}

impl Resource {
    /// Create a `Mail` instance representing this `Resource`.
    ///
    /// This is not a complete mail, i.e. it will not contain
    /// headers like `From` or `To` and in many cases the
    /// returned `Mail` instance will be wrapped into other
    /// mail instances adding alternative bodies, embedded
    /// resources and attachments.
    pub fn create_mail(self) -> Mail {
        Mail::new_singlepart_mail(self)
    }

    pub fn create_mail_with_disposition(self, disposition_kind: DispositionKind) -> Mail {
        let mut mail = self.create_mail();
        //TODO[1.0] grab meta from resource
        let disposition = Disposition::new(disposition_kind, Default::default());
        mail.insert_header(headers::ContentDisposition::body(disposition));
        mail
    }
}

impl Mail {
    /// Create a `multipart/mixed` `Mail` instance containing this mail as
    /// first body and one additional body for each attachment.
    ///
    /// Normally this is used with embeddings having a attachment
    /// disposition creating a mail with attachments.
    pub fn wrap_with_mixed(self, other_bodies: Vec<Mail>) -> Mail {
        let mut bodies = other_bodies;
        bodies.push(self);
        new_multipart(&MIXED, bodies)
    }

    /// Create a `multipart/alternative` `Mail` instance containing this
    /// mail as the _main_ body with given alternatives.
    ///
    /// The "priority" of alternative bodies is ascending with the body
    /// which should be shown only if all other bodies can't be displayed
    /// first. I.e. the order is the same order as
    /// specified by `multipart/alternative`.
    /// This also means that _this_ body will be the last body as it is
    /// meant to be the _main_ body.
    pub fn wrap_with_alternatives(self, alternates: Vec<Mail>) -> Mail {
        let mut bodies = alternates;
        //TODO[opt] accept iter and prepend instead of insert in vec
        bodies.insert(0, self);
        new_multipart(&ALTERNATIVE, bodies)
    }

    /// Creates a `multipart/related` `Mail` instance containing this
    /// mail first and then all related bodies.
    pub fn wrap_with_related(self, related: Vec<Mail>) -> Mail {
        let mut bodies = related;
        bodies.insert(0, self);
        new_multipart(&RELATED, bodies)
    }
}

/// Creates a `multipart/<sub_type>` mail with given bodies.
///
/// # Panic
///
/// If `sub_type` can not be used to create a multipart content
/// type this will panic.
fn new_multipart(sub_type: &'static str, bodies: Vec<Mail>) -> Mail {
    let content_type = MediaType::new(MULTIPART, sub_type).unwrap();
    Mail::new_multipart_mail(content_type, bodies)
}
