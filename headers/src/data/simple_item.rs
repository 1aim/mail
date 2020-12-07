use std::ops::Deref;

use soft_ascii_string::{SoftAsciiStr, SoftAsciiString};

use super::inner_item::{InnerAscii, InnerUtf8};
use super::input::Input;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum SimpleItem {
    /// specifies that the Item is valid Ascii, nothing more
    Ascii(InnerAscii),
    /// specifies that the Item is valid Utf8, nothing more
    Utf8(InnerUtf8),
}

impl SimpleItem {
    pub fn as_str(&self) -> &str {
        use self::SimpleItem::*;
        match *self {
            Ascii(ref value) => value.as_str(),
            Utf8(ref value) => value.as_str(),
        }
    }

    pub fn is_ascii(&self) -> bool {
        use self::SimpleItem::*;
        match *self {
            Ascii(..) => true,
            Utf8(..) => false,
        }
    }

    pub fn from_utf8_input(s: Input) -> Self {
        SimpleItem::Utf8(s.0)
    }

    pub fn from_utf8(s: String) -> Self {
        SimpleItem::Utf8(InnerUtf8::Owned(s))
    }
}

impl Deref for SimpleItem {
    type Target = str;

    fn deref(&self) -> &str {
        use self::SimpleItem::*;
        match *self {
            Ascii(ref astr) => astr.as_str(),
            Utf8(ref utf8) => &**utf8,
        }
    }
}

impl Into<String> for SimpleItem {
    fn into(self) -> String {
        use self::SimpleItem::*;
        match self {
            Ascii(aitem) => {
                let astring: SoftAsciiString = aitem.into();
                astring.into()
            }
            Utf8(string) => string.into(),
        }
    }
}

impl<'a> From<&'a str> for SimpleItem {
    fn from(string: &'a str) -> Self {
        Self::from(String::from(string))
    }
}

impl From<String> for SimpleItem {
    fn from(string: String) -> Self {
        match SoftAsciiString::from_string(string) {
            Ok(astring) => SimpleItem::Ascii(InnerAscii::Owned(astring)),
            Err(err) => SimpleItem::Utf8(InnerUtf8::Owned(err.into_source())),
        }
    }
}

impl From<SoftAsciiString> for SimpleItem {
    fn from(astring: SoftAsciiString) -> Self {
        SimpleItem::Ascii(InnerAscii::Owned(astring))
    }
}

impl From<Input> for SimpleItem {
    fn from(input: Input) -> Self {
        match input {
            Input(InnerUtf8::Owned(string)) => match SoftAsciiString::from_string(string) {
                Ok(ascii) => SimpleItem::Ascii(InnerAscii::Owned(ascii)),
                Err(err) => SimpleItem::Utf8(InnerUtf8::Owned(err.into_source())),
            },
            Input(InnerUtf8::Shared(shared)) => {
                if shared.is_ascii() {
                    let a_shared = shared.map(|s| SoftAsciiStr::from_unchecked(s));
                    SimpleItem::Ascii(InnerAscii::Shared(a_shared))
                } else {
                    SimpleItem::Utf8(InnerUtf8::Shared(shared))
                }
            }
        }
    }
}
