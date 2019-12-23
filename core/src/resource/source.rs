use headers::header_components::MediaType;
use iri::IRI;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// POD containing the IRI which should be used to laod a resource well as
/// an optional file name to use and a description about how the content type
/// should be handled.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Source {
    /// A International Resource Identifier pointing to a source
    /// from which the Resource can be loaded. Note that the interpretation
    /// of the IRI is left to the `ResourceLoader` implementation of the
    /// context. The `ResourceLoader` can decide to reject valid IRI's e.g.
    /// a (non local) http url is likely to be rejected by any implementation.
    pub iri: IRI,

    /// Allows specifying how the media type detection is done.
    #[cfg_attr(feature = "serde", serde(default))]
    pub use_media_type: UseMediaType,

    /// Allows providing a explicit name overriding any inferred name.
    ///
    /// If a resource if loaded from a IRI it potentially can contain
    /// a inferred name e.g. for loading a file `secret_thing.png` it
    /// would be just that file name, but you potentially want to provide
    /// a name which differs from the name the file has in the file system
    /// in which case you can provide a name here.
    ///
    /// Note that file names are optional and don't need to be provided at all.
    /// But it is strongly recommended to provide them for anything used as
    /// attachment but normally irrelevant for anything else.
    #[cfg_attr(feature = "serde", serde(default))]
    pub use_file_name: Option<String>,
}

/// Specifies how the content type should be handled when loading the data.
///
/// Depending on how the context implementation handles the loading it might
/// already have a content type, but if not it might also need to "sniff" it,
/// which can fail. Nevertheless how any of the aspects are handled in detail
/// depends on the context implementation.
///
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum UseMediaType {
    /// Sniff content type if no content type was given from any other place.
    Auto,

    /// Use this content type if no content type was given from any other place.
    ///
    /// As resources are loaded by IRI they could be loaded from any place and
    /// this place could be storing the data with the right content type, in which
    /// case that content type should be used.
    Default(MediaType),
    // /// Always use this content type even if it is known to have a different content type.
    // Override(MediaType)
}

impl Default for UseMediaType {
    fn default() -> Self {
        UseMediaType::Auto
    }
}
