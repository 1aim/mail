use std::{
    default::Default,
    ops::{Deref, DerefMut},
    sync::Arc,
};

#[cfg(feature = "serde")]
use serde::{de::Deserializer, ser::Serializer, Deserialize, Serialize};

use headers::header_components::{ContentId, FileMeta, MediaType, TransferEncoding};
use internals::bind::{base64, quoted_printable};

/// POD type containing FileMeta, Content-Type and Content-Id
///
/// The file meta contains optional information like file name and read
/// as well as last modification data.
///
/// The media type will be used for the content type header which is used
/// to determine how a mail client will handle the file. It is also used
/// to get a hint on how to best transfer encode the file.
///
/// The content id is used to identify the "data" and refer to it
/// from some other place. For example in a mail the html body could
/// refer to a image contained in the mail to embed it in the mail.
///
/// As Content-Id's are supposed to be world unique they could also
/// be used for some caching and similar but that plays hardly any
/// role any more, except maybe for "external" mail bodies.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Metadata {
    /// File meta like file name or file read time.
    #[cfg_attr(feature = "serde", serde(flatten))]
    pub file_meta: FileMeta,

    /// The media type of the data.
    pub media_type: MediaType,

    /// The content id associated with the data.
    pub content_id: ContentId,
}

impl Deref for Metadata {
    type Target = FileMeta;

    fn deref(&self) -> &Self::Target {
        &self.file_meta
    }
}

impl DerefMut for Metadata {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.file_meta
    }
}

/// A type containing some data and metadata for it.
///
/// This often, but not always, corresponds to data which could potentially
/// have been a file in a file system. For example a image or a text
/// document.
///
/// This type is mainly used when having auto generated content as content
/// provided through a file should be loaded from a source and as such
/// will be directly loaded and transfer encoded.
///
/// # Clone
///
/// `Data` is made to be cheap to clone and share.
/// For this it uses `Arc` internally.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Data {
    #[cfg_attr(feature = "serde", serde(with = "arc_buffer_serde"))]
    buffer: Arc<[u8]>,
    #[cfg_attr(feature = "serde", serde(flatten))]
    #[cfg_attr(feature = "serde", serde(with = "arc_serde"))]
    meta: Arc<Metadata>,
}

impl Data {
    /// Create a new data instance.
    pub fn new(buffer: impl Into<Arc<[u8]>>, meta: impl Into<Arc<Metadata>>) -> Self {
        Data {
            buffer: buffer.into(),
            meta: meta.into(),
        }
    }

    pub fn plain_text(text: impl Into<String>, cid: ContentId) -> Data {
        let text = text.into();
        let buf = text.into_bytes();
        let meta = Metadata {
            file_meta: Default::default(),
            media_type: MediaType::parse("text/plain; charset=utf-8").unwrap(),
            content_id: cid,
        };
        Self::new(buf, meta)
    }

    /// Access the raw data buffer of this instance.
    pub fn buffer(&self) -> &Arc<[u8]> {
        &self.buffer
    }

    /// Access the metadata.
    pub fn metadata(&self) -> &Arc<Metadata> {
        &self.meta
    }

    /// Access the file meta metadata.Fn
    pub fn file_meta(&self) -> &FileMeta {
        &self.meta.file_meta
    }

    /// Access the content type.
    pub fn media_type(&self) -> &MediaType {
        &self.meta.media_type
    }

    /// Access the content id.
    pub fn content_id(&self) -> &ContentId {
        &self.meta.content_id
    }

    /// Transfer encode the given data.
    ///
    /// This function will be called by the context implementation when
    /// loading and/or transfer encoding data. The context implementation
    /// might also not call it if it has a cached version of the transfer
    /// encoded data.
    ///
    /// This functions expect a boundary pool and will remove all boundaries
    /// which do appear in the encoded representation of the data.
    #[inline(always)]
    pub fn transfer_encode(&self, encoding_hint: TransferEncodingHint) -> EncData {
        // delegated to free function at end of file for
        // readability
        transfer_encode(self, encoding_hint)
    }
}

/// `EncData` is like `Data` but the buffer contains transfer encoded data.
///
/// # Clone
///
/// `Data` is made to be cheap to clone and share.
/// For this it uses `Arc` internally.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct EncData {
    #[cfg_attr(feature = "serde", serde(with = "arc_buffer_serde"))]
    buffer: Arc<[u8]>,
    #[cfg_attr(feature = "serde", serde(flatten))]
    #[cfg_attr(feature = "serde", serde(with = "arc_serde"))]
    meta: Arc<Metadata>,
    encoding: TransferEncoding,
}

impl EncData {
    /// Create a new instance from transfer encoded data
    /// as well as metadata and the encoding used to transfer
    /// encode the data.
    ///
    /// If the `buffer` was created by transfer encoding data
    /// from a `Data` instance the `Arc<Metadata>` from that
    /// `Data` instance can be passed in directly as `meta`.
    pub(crate) fn new(
        buffer: impl Into<Arc<[u8]>>,
        meta: impl Into<Arc<Metadata>>,
        encoding: TransferEncoding,
    ) -> Self {
        EncData {
            buffer: buffer.into(),
            meta: meta.into(),
            encoding,
        }
    }

    /// Access the raw transfer encoded data.
    pub fn transfer_encoded_buffer(&self) -> &Arc<[u8]> {
        &self.buffer
    }

    /// Access the metadata.
    pub fn metadata(&self) -> &Arc<Metadata> {
        &self.meta
    }

    /// Access the file meta metadata.Fn
    pub fn file_meta(&self) -> &FileMeta {
        &self.meta.file_meta
    }

    /// Access the content type.
    pub fn media_type(&self) -> &MediaType {
        &self.meta.media_type
    }

    /// Access the transfer encoding used to encode the buffer.
    pub fn encoding(&self) -> TransferEncoding {
        self.encoding
    }

    /// Access the content id.
    ///
    /// The content id is for the data itself so it should not
    /// change just because the data had been transfer encoded.
    ///
    /// # Note about fixed newlines:
    ///
    /// The encoding functions of this library will always "fix"
    /// line endings even if the transfer encoding is to not have
    /// any encoding, it could be said that this is a modification
    /// of the data and as such the content id should change. But
    /// as this is done _always_ and as such only the transfer encoded
    /// data is "send" out this works out fine.
    pub fn content_id(&self) -> &ContentId {
        &self.meta.content_id
    }
}

/// Hint to change how data should be transfer encoded.
#[derive(Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum TransferEncodingHint {
    /// Use Base64 encoding.
    UseBase64,

    /// Use Quoted-Printable encoding.
    UseQuotedPrintable,

    // /// Do not assume Mime8Bit is available.
    // ///
    // /// As such do not encode ascii/utf-8 "as is" (e.g. not encoding them).
    // ///
    // /// Note: This is the default until I'm more sure about the whole thing
    // /// with puthing things in unecoded.
    // DoNotUseNoEncoding,
    /// No hint for transfer encoding.
    NoHint,

    #[cfg_attr(feature = "serde", serde(skip))]
    #[doc(hidden)]
    __NonExhaustive {},
}

impl Default for TransferEncodingHint {
    fn default() -> Self {
        TransferEncodingHint::UseBase64
    }
}

/// Transfer encodes Data.
///
/// Util we have a reasonable "non latin letter text" heuristic
/// or enable none encoded text as default this will always encode
/// with `Base64` except if asked not to do so.
///
/// # Panic
///
/// Panics if TransferEncodingHint::__NonExhaustive
/// is passed to the function.
fn transfer_encode(data: &Data, encoding_hint: TransferEncodingHint) -> EncData {
    use self::TransferEncodingHint::*;

    match encoding_hint {
        UseQuotedPrintable => tenc_quoted_printable(data),
        UseBase64 | NoHint => tenc_base64(data),
        __NonExhaustive { .. } => {
            panic!("__NonExhaustive encoding should not be passed to any place")
        }
    }
}

fn tenc_base64(data: &Data) -> EncData {
    let enc_data = base64::normal_encode(data.buffer()).into_bytes();

    EncData::new(enc_data, data.metadata().clone(), TransferEncoding::Base64)
}

fn tenc_quoted_printable(data: &Data) -> EncData {
    let enc_data = quoted_printable::normal_encode(data.buffer()).into_bytes();

    EncData::new(
        enc_data,
        data.metadata().clone(),
        TransferEncoding::QuotedPrintable,
    )
}

#[cfg(feature = "serde")]
mod arc_buffer_serde {
    use super::*;

    pub(crate) fn deserialize<'de, D>(deserializer: D) -> Result<Arc<[u8]>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let bytes = <Vec<u8>>::deserialize(deserializer)?;
        Ok(bytes.into())
    }

    pub(crate) fn serialize<S>(data: &Arc<[u8]>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_bytes(data)
    }
}

#[cfg(feature = "serde")]
mod arc_serde {
    use super::*;

    pub(crate) fn deserialize<'de, OUT, D>(deserializer: D) -> Result<Arc<OUT>, D::Error>
    where
        D: Deserializer<'de>,
        OUT: Deserialize<'de>,
    {
        let value = OUT::deserialize(deserializer)?;
        Ok(Arc::new(value))
    }

    pub(crate) fn serialize<S, IN>(data: &Arc<IN>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
        IN: Serialize,
    {
        IN::serialize(&**data, serializer)
    }
}
