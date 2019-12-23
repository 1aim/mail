//! Module containing all custom errors produced by this crate.
use std::fmt::{self, Display};
use std::io;

use failure::{Backtrace, Context, Fail};

use headers::error::{
    BuildInValidationError, ComponentCreationError, HeaderTypeError, HeaderValidationError,
};
use internals::error::EncodingError;
use IRI;
// errors from loading a Resource (which includes encoding it's body)
//                /  NotFound       | IRI (no Backtrace neede)     \ MailError::ResourceLoading
// ResourceError <   LoadingFailed  | chain Error                  /
//                \  EncodingFailed | EncodingError (Backtrace!)   > MailError::Encoding
//

/// Error caused by failing to load an `Resource`
///
#[derive(Debug, Fail)]
pub enum ResourceError {
    /// The loading on itself failed.
    #[fail(display = "{}", _0)]
    Loading(ResourceLoadingError),

    /// The encoding of the resource failed.
    ///
    /// Note: Resources are encoded as this allows shared
    /// resources to not be re-encoded every time they are
    /// used.
    #[fail(display = "{}", _0)]
    Encoding(EncodingError),
}

impl From<EncodingError> for ResourceError {
    fn from(err: EncodingError) -> Self {
        ResourceError::Encoding(err)
    }
}

impl From<ResourceLoadingError> for ResourceError {
    fn from(err: ResourceLoadingError) -> Self {
        ResourceError::Loading(err)
    }
}

/// Reasons why the loading of an `Resource` can fail.
#[derive(Copy, Clone, Debug, Fail, PartialEq, Eq, Hash)]
pub enum ResourceLoadingErrorKind {
    /// The resource wasn't found.
    #[fail(display = "resource not found")]
    NotFound,

    /// The act of loading it failed (e.g. because of an I/0-Error)
    #[fail(display = "loading failed")]
    LoadingFailed,

    #[fail(display = "automatically detecting the media type failed")]
    MediaTypeDetectionFailed,
}

/// The loading of an Resource failed.
#[derive(Debug)]
pub struct ResourceLoadingError {
    inner: Context<ResourceLoadingErrorKind>,
    iri: Option<IRI>,
}

impl Display for ResourceLoadingError {
    fn fmt(&self, fter: &mut fmt::Formatter) -> fmt::Result {
        Display::fmt(&self.inner, fter)
    }
}

impl Fail for ResourceLoadingError {
    fn cause(&self) -> Option<&Fail> {
        self.inner.cause()
    }

    fn backtrace(&self) -> Option<&Backtrace> {
        self.inner.backtrace()
    }
}

impl ResourceLoadingError {
    /// The kind of error which caused the loading to fail.
    pub fn kind(&self) -> ResourceLoadingErrorKind {
        *self.inner.get_context()
    }

    /// The source IRI which was used when failing to load the Resource.
    pub fn source_iri(&self) -> Option<&IRI> {
        self.iri.as_ref()
    }

    /// Sets the source IRI if not already set and returns self.
    pub fn with_source_iri_or_else<F>(mut self, func: F) -> Self
    where
        F: FnOnce() -> Option<IRI>,
    {
        if self.iri.is_none() {
            self.iri = func();
        }
        self
    }
}

impl From<ResourceLoadingErrorKind> for ResourceLoadingError {
    fn from(err: ResourceLoadingErrorKind) -> Self {
        ResourceLoadingError::from((None, err))
    }
}

impl From<Context<ResourceLoadingErrorKind>> for ResourceLoadingError {
    fn from(inner: Context<ResourceLoadingErrorKind>) -> Self {
        ResourceLoadingError::from((None, inner))
    }
}

impl From<(IRI, ResourceLoadingErrorKind)> for ResourceLoadingError {
    fn from((iri, error_kind): (IRI, ResourceLoadingErrorKind)) -> Self {
        ResourceLoadingError::from((Some(iri), error_kind))
    }
}

impl From<(IRI, Context<ResourceLoadingErrorKind>)> for ResourceLoadingError {
    fn from((iri, inner): (IRI, Context<ResourceLoadingErrorKind>)) -> Self {
        ResourceLoadingError::from((Some(iri), inner))
    }
}

impl From<(Option<IRI>, ResourceLoadingErrorKind)> for ResourceLoadingError {
    fn from((iri, error_kind): (Option<IRI>, ResourceLoadingErrorKind)) -> Self {
        ResourceLoadingError::from((iri, Context::new(error_kind)))
    }
}

impl From<(Option<IRI>, Context<ResourceLoadingErrorKind>)> for ResourceLoadingError {
    fn from((iri, inner): (Option<IRI>, Context<ResourceLoadingErrorKind>)) -> Self {
        ResourceLoadingError { inner, iri }
    }
}

impl From<io::Error> for ResourceLoadingError {
    fn from(err: io::Error) -> Self {
        err.context(ResourceLoadingErrorKind::LoadingFailed).into()
    }
}

#[derive(Debug, Fail)]
pub enum OtherValidationError {
    /// Non-multipart mail headers derive the Content-Type header from it's body `Resource`.
    ///
    /// This error is returned if a `Content-Type` header was given never the less.
    #[fail(display = "Content-Type header given for non multipart mail")]
    ContentTypeHeaderGiven,

    /// `Content-Transfer-Encoding` headers are always auto-generated
    /// and can not be manually set.
    #[fail(display = "Content-Transfer-Encoding header given")]
    ContentTransferEncodingHeaderGiven,

    /// A non "multipart" media type was given as content type for a multipart mail.
    #[fail(display = "found non multipart content type in multipart mail")]
    SingleMultipartMixup,

    /// Inserting a `Conent-Type` header into a singlepart body is not allowed.
    ///
    /// In single-part bodies the `Content-Type` header is always auto-generated
    /// based on the actual body.
    #[fail(display = "inserting Content-Type for singlepart body is not allowed")]
    InsertSinglepartContentTypeHeader,

    /// A multipart mail requires a `Content-Type` header to be given.
    #[fail(display = "multipart mail does not contain a content type header")]
    MissingContentTypeHeader,

    /// A mail (top level, not in multipart) requires a `From` header to be given.
    #[fail(display = "mail did not contain a From header")]
    NoFrom,
}

impl From<OtherValidationError> for HeaderValidationError {
    fn from(oe: OtherValidationError) -> Self {
        let err: ::failure::Error = oe.into();
        HeaderValidationError::Custom(err)
    }
}

impl From<OtherValidationError> for MailError {
    fn from(oe: OtherValidationError) -> Self {
        let val_err = HeaderValidationError::from(oe);
        MailError::from(val_err)
    }
}

/// General Error combining most other error wrt. mail creation and encoding.
#[derive(Debug, Fail)]
pub enum MailError {
    /// Encoding the mail failed.
    #[fail(display = "{}", _0)]
    Encoding(EncodingError),

    /// Different implementations for the same header where mixed up.
    #[fail(display = "{}", _0)]
    Type(HeaderTypeError),

    /// Creating a mail header body (component) failed.
    #[fail(display = "{}", _0)]
    Component(ComponentCreationError),

    /// The mail has some invalid header or header combinations.
    ///
    /// E.g. it has a `From` header with multiple mailboxes but no
    /// `Sender` header (which is only required if `From` has more
    /// than one mailbox).
    #[fail(display = "{}", _0)]
    Validation(HeaderValidationError),

    /// Loading an resource failed.
    ///
    /// E.g. the file to attach or the image to embedded could not
    /// be found.
    #[fail(display = "{}", _0)]
    ResourceLoading(ResourceLoadingError),
}

impl From<BuildInValidationError> for MailError {
    fn from(err: BuildInValidationError) -> Self {
        MailError::Validation(err.into())
    }
}

impl From<HeaderTypeError> for MailError {
    fn from(err: HeaderTypeError) -> Self {
        MailError::Type(err)
    }
}

impl From<EncodingError> for MailError {
    fn from(err: EncodingError) -> Self {
        MailError::Encoding(err)
    }
}

impl From<HeaderValidationError> for MailError {
    fn from(err: HeaderValidationError) -> Self {
        MailError::Validation(err)
    }
}

impl From<ResourceLoadingError> for MailError {
    fn from(err: ResourceLoadingError) -> Self {
        MailError::ResourceLoading(err)
    }
}

impl From<ResourceError> for MailError {
    fn from(err: ResourceError) -> Self {
        match err {
            ResourceError::Loading(err) => MailError::ResourceLoading(err),
            ResourceError::Encoding(err) => MailError::Encoding(err),
        }
    }
}

impl From<ComponentCreationError> for MailError {
    fn from(err: ComponentCreationError) -> Self {
        MailError::Component(err)
    }
}

/// Error returned when trying to _unload_ and `Resource` and it fails.
#[derive(Copy, Clone, Debug, Fail)]
pub enum ResourceNotUnloadableError {
    /// The resource can not be unloaded because its in use.
    #[fail(display = "resource is in use, can't unload it")]
    InUse,
    /// The resource can not be unloaded because it doesn't has a source.
    ///
    /// Which means if we would unload it we could not reload it. Note
    /// that unloading is just for thinks like caching, it doesn't affect
    /// the deletion/dropping of `Resource` instances.
    #[fail(display = "resource has no source, can't unload it")]
    NoSource,
}
