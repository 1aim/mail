use std::borrow::ToOwned;
use std::ops::Deref;
use std::sync::Arc;

use owning_ref::OwningRef;
use soft_ascii_string::{SoftAsciiStr, SoftAsciiString};

#[cfg(feature = "serde")]
use serde::{de::Error as __Error, Deserialize, Deserializer, Serialize, Serializer};

/// InnerAscii is string data container which can contain either a
/// owned `SoftAsciiString` or a `SoftAsciiStr` reference into a shared
/// string buffer.
#[derive(Debug, Clone, Hash, Eq)]
pub enum InnerAscii {
    Owned(SoftAsciiString),
    //by using String+SoftAsciiStr we can eliminate unessesary copies
    Shared(OwningRef<Arc<String>, SoftAsciiStr>),
}

impl InnerAscii {
    /// converts this container into on which uses underlying shared data
    ///
    /// if the data is already shared nothing is done.
    /// If not the owned data is converted into the underlying string buffer
    /// and `OwningRef` is used to enable the shared reference
    ///
    /// Note that the underlying buffer is no an `SoftAsciiString` but a
    /// `String` (from which we happend to know that it fulfills the "is
    ///  us-ascii" soft constraint). This allows us to have an `InnerAscii`
    /// share data with a possible non us-ascii string buffer as long as
    /// the part accessable through the `SoftAsciiStr` is ascii. Or at last
    /// should be ascii as it's a soft constraint.
    pub fn into_shared(self) -> Self {
        match self {
            InnerAscii::Owned(value) => {
                let buffer: Arc<String> = Arc::new(value.into());
                let orf = OwningRef::new(buffer).map(|data: &String| {
                    // we got it from a SoftAsciiString so no check here
                    SoftAsciiStr::from_unchecked(&**data)
                });
                InnerAscii::Shared(orf)
            }
            v => v,
        }
    }
}

/// InnerUtf8 is string data container which can contain either a
/// owned `String` or a `str` reference into a shared
/// string buffer.
#[derive(Debug, Clone, Hash, Eq)]
pub enum InnerUtf8 {
    Owned(String),
    //by using String+SoftAsciiStr we can eliminate unessesary copies
    Shared(OwningRef<Arc<String>, str>),
}

impl InnerUtf8 {
    /// converts this container into on which uses underlying shared data
    ///
    /// if the data is already shared nothing is done.
    /// If not the owned data is converted into the underlying string buffer
    /// and `OwningRef` is used to enable the shared reference
    pub fn into_shared(self) -> Self {
        match self {
            InnerUtf8::Owned(value) => {
                let buffer = Arc::new(value);
                let orf = OwningRef::new(buffer).map(|rced| &**rced);
                InnerUtf8::Shared(orf)
            }
            v => v,
        }
    }
}

macro_rules! inner_impl {
    ($name:ident, $owned_form:ty, $borrowed_form:ty) => {
        impl $name {
            pub fn new<S: Into<$owned_form>>(data: S) -> Self {
                $name::Owned(data.into())
            }
        }
        impl From<$owned_form> for $name {
            fn from(data: $owned_form) -> Self {
                Self::new(data)
            }
        }

        impl Into<$owned_form> for $name {
            fn into(self) -> $owned_form {
                match self {
                    $name::Owned(owned) => owned,
                    $name::Shared(shared) => {
                        let as_ref: &$borrowed_form = &*shared;
                        as_ref.to_owned()
                    }
                }
            }
        }

        impl Deref for $name {
            type Target = $borrowed_form;

            fn deref(&self) -> &$borrowed_form {
                match *self {
                    $name::Owned(ref string) => &*string,
                    $name::Shared(ref owning_ref) => &*owning_ref,
                }
            }
        }

        #[cfg(feature = "serde")]
        impl Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                let borrowed: &$borrowed_form = &*self;
                let as_ref: &str = borrowed.as_ref();
                serializer.serialize_str(as_ref)
            }
        }

        impl PartialEq for $name {
            fn eq(&self, other: &$name) -> bool {
                let me: &$borrowed_form = &*self;
                let other: &$borrowed_form = &*other;
                me == other
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                self.as_str()
            }
        }
    };
}

inner_impl! { InnerAscii, SoftAsciiString,  SoftAsciiStr }
inner_impl! { InnerUtf8, String, str }
//inner_impl!{ InnerOtherItem, OtherString, OtherStr }

impl InnerAscii {
    pub fn as_str(&self) -> &str {
        match *self {
            InnerAscii::Owned(ref owned) => owned.as_str(),
            InnerAscii::Shared(ref shared) => shared.as_str(),
        }
    }
}

#[cfg(feature = "serde")]
impl<'de> Deserialize<'de> for InnerAscii {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let content = String::deserialize(deserializer).map_err(|err| D::Error::custom(err))?;
        let content = SoftAsciiString::from_string(content).map_err(|err| D::Error::custom(err))?;
        Ok(InnerAscii::from(content))
    }
}

impl InnerUtf8 {
    pub fn as_str(&self) -> &str {
        match *self {
            InnerUtf8::Owned(ref owned) => owned.as_str(),
            InnerUtf8::Shared(ref shared) => &**shared,
        }
    }
}

#[cfg(feature = "serde")]
impl<'de> Deserialize<'de> for InnerUtf8 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let content = String::deserialize(deserializer).map_err(|err| D::Error::custom(err))?;
        Ok(InnerUtf8::from(content))
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn inner_ascii_item_eq() {
        let a = InnerAscii::Owned(SoftAsciiString::from_string("same").unwrap());
        let b = InnerAscii::Shared(
            OwningRef::new(Arc::new("same".to_owned())).map(|v| SoftAsciiStr::from_unchecked(&**v)),
        );
        assert_eq!(a, b);
    }

    #[test]
    fn inner_ascii_item_neq() {
        let a = InnerAscii::Owned(SoftAsciiString::from_string("same").unwrap());
        let b = InnerAscii::Shared(
            OwningRef::new(Arc::new("not same".to_owned()))
                .map(|v| SoftAsciiStr::from_unchecked(&**v)),
        );
        assert_ne!(a, b);
    }

    #[test]
    fn inner_utf8_item_eq() {
        let a = InnerUtf8::Owned(String::from("same"));
        let b = InnerUtf8::Shared(OwningRef::new(Arc::new(String::from("same"))).map(|v| &**v));
        assert_eq!(a, b);
    }

    #[test]
    fn inner_utf8_item_neq() {
        let a = InnerUtf8::Owned(String::from("same"));
        let b = InnerUtf8::Shared(OwningRef::new(Arc::new(String::from("not same"))).map(|v| &**v));
        assert_ne!(a, b);
    }

    #[test]
    fn has_as_str() {
        use std::borrow::ToOwned;

        assert_eq!(
            "hy",
            InnerAscii::Owned(SoftAsciiStr::from_unchecked("hy").to_owned()).as_str()
        );
        assert_eq!("hy", InnerUtf8::Owned("hy".into()).as_str());
    }
}
