use chrono::DateTime;
use chrono::Utc;

use std::mem::replace;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// A struct representing common file metadata.
///
/// This is used by e.g. attachments, when attaching
/// a file (or embedding an image). Through it's usage
/// is optional.
///
/// # Stability Note
///
/// This is likely to move to an different place at
/// some point, potentially in a different `mail-*`
/// crate.
#[derive(Debug, Clone, Eq, PartialEq, Hash, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct FileMeta {
    /// The file name.
    ///
    /// Note that this utility is limited to utf-8 file names.
    /// This is normally used when downloading a attachment to
    /// choose the default file name.
    #[cfg_attr(feature = "serde", serde(default))]
    pub file_name: Option<String>,

    /// The creation date of the file (in utc).
    #[cfg_attr(feature = "serde", serde(default))]
    #[cfg_attr(feature = "serde", serde(with = "super::utils::serde::opt_date_time"))]
    pub creation_date: Option<DateTime<Utc>>,

    /// The last modification date of the file (in utc).
    #[cfg_attr(feature = "serde", serde(default))]
    #[cfg_attr(feature = "serde", serde(with = "super::utils::serde::opt_date_time"))]
    pub modification_date: Option<DateTime<Utc>>,

    /// The date time the file was read, i.e. placed in the mail (in utc).
    #[cfg_attr(feature = "serde", serde(default))]
    #[cfg_attr(feature = "serde", serde(with = "super::utils::serde::opt_date_time"))]
    pub read_date: Option<DateTime<Utc>>,

    /// The size the file should have.
    ///
    /// Note that normally mail explicitly opts to NOT specify the size
    /// of a mime-multi part body (e.g. an attachments) and you can never
    /// rely on it to e.g. skip ahead. But it has some uses wrt. thinks
    /// like external headers.
    #[cfg_attr(feature = "serde", serde(default))]
    pub size: Option<usize>,
}

macro_rules! impl_replace_none {
    ($self_:expr, $other:expr, [$($field:ident),*]) => ({
        let &mut FileMeta {$(
            ref mut $field
        ),*} = $self_;

        $(
            if $field.is_none() {
                replace($field, $other.$field.clone());
            }
        )*
    })
}

impl FileMeta {
    /// Replaces all fields which are `None` with the value of the field in `other_meta`.
    pub fn replace_empty_fields_with(&mut self, other_meta: &Self) {
        impl_replace_none! {
            self, other_meta,
            [file_name, creation_date, modification_date, read_date, size]
        }
    }
}
