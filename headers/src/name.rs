use soft_ascii_string::SoftAsciiStr;
use std::fmt;

use internals::grammar::is_ftext;

///
/// Note: Normally you will never have the need to create a HeaderName instance by
/// yourself (except maybe for testing). At last as long as you use `def_header!`
/// for defining custom Headers, which is highly recommended
///
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub struct HeaderName {
    name: &'static SoftAsciiStr,
}

impl HeaderName {
    ///
    /// Be aware, that this library only accepts header names with a letter case,
    /// that any first character of an alphanumeric part of a header name has to
    /// be uppercase and all other lowercase. E.g. `Message-Id` is accepted but
    /// `Message-ID` is rejected, even through both are _semantically_ the same.
    /// This frees us from doing either case insensitive comparison/hash wrt. hash map
    /// lookups, or converting all names to upper/lower case.
    ///
    pub fn new(name: &'static SoftAsciiStr) -> Result<Self, InvalidHeaderName> {
        HeaderName::validate_name(name)?;
        Ok(HeaderName { name })
    }

    pub fn from_ascii_unchecked<B: ?Sized>(name: &'static B) -> HeaderName
    where
        B: AsRef<str>,
    {
        HeaderName {
            name: SoftAsciiStr::from_unchecked(name.as_ref()),
        }
    }

    #[inline(always)]
    pub fn as_ascii_str(&self) -> &'static SoftAsciiStr {
        self.name
    }
    #[inline(always)]
    pub fn as_str(&self) -> &'static str {
        self.name.as_str()
    }
}

impl fmt::Display for HeaderName {
    fn fmt(&self, fter: &mut fmt::Formatter) -> fmt::Result {
        write!(fter, "{}", self.as_str())
    }
}

impl PartialEq<str> for HeaderName {
    fn eq(&self, other: &str) -> bool {
        self.name.as_str() == other
    }
}

impl PartialEq<SoftAsciiStr> for HeaderName {
    fn eq(&self, other: &SoftAsciiStr) -> bool {
        self.name == other
    }
}

impl HeaderName {
    /// validates if the header name is valid
    ///
    /// by only allowing names in "snake case" no case
    /// insensitive comparison or case conversion is needed
    /// for header names
    fn validate_name(name: &SoftAsciiStr) -> Result<(), InvalidHeaderName> {
        let mut begin_of_word = true;
        if name.len() < 1 {
            return Err(InvalidHeaderName {
                invalid_name: name.to_owned().into(),
            });
        }

        for ch in name.as_str().chars() {
            if !is_ftext(ch) {
                return Err(InvalidHeaderName {
                    invalid_name: name.to_owned().into(),
                });
            }
            match ch {
                'a'...'z' => {
                    if begin_of_word {
                        return Err(InvalidHeaderName {
                            invalid_name: name.to_owned().into(),
                        });
                    }
                }
                'A'...'Z' => {
                    if begin_of_word {
                        begin_of_word = false;
                    } else {
                        return Err(InvalidHeaderName {
                            invalid_name: name.to_owned().into(),
                        });
                    }
                }
                '0'...'9' => {
                    begin_of_word = false;
                }
                ch => {
                    if ch < '!' || ch > '~' || ch == ':' {
                        return Err(InvalidHeaderName {
                            invalid_name: name.to_owned().into(),
                        });
                    }
                    begin_of_word = true;
                }
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Fail)]
#[fail(display = "given name is not a valid header name: {:?}", invalid_name)]
pub struct InvalidHeaderName {
    invalid_name: String,
}

/// a utility trait allowing us to use type hint structs
/// in `HeaderMap::{contains, get_untyped}`
pub trait HasHeaderName {
    fn get_name(&self) -> HeaderName;
}

impl HasHeaderName for HeaderName {
    fn get_name(&self) -> HeaderName {
        *self
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn valide_header_names() {
        let valid_cases = &[
            "Date",
            "Some-Header",
            "33",
            "Some34",
            // even trough they seem wrong the email standard only states
            // header field names have to be at last one char and can
            // only consist of printable US-ACII chars without :
            // meaning e.g. "<3" is as valide as "3*4=12"
            "-33-",
            "---",
            "<3+Who-Cares&44",
            "(3*4=12)^[{~}]",
        ];
        for case in valid_cases.iter() {
            assert_ok!(HeaderName::validate_name(
                SoftAsciiStr::from_str(case).unwrap()
            ));
        }
    }

    #[test]
    fn invalide_header_names() {
        // we only alow "snake case" like names to not have to do
        // case insensitive comparsion in hashmap lookups
        let invalid_cases = &[
            "ID",
            "DaD",
            "ans",
            "all-lower-calse",
            "ALL-UPPER-CASE",
            "",
            "a:b",
            ":",
            "-:-",
            "Message Id",
            " Leading-Ws",
            "Message\tId",
            "Null\0Msg",
        ];
        for case in invalid_cases.iter() {
            assert_err!(
                HeaderName::validate_name(SoftAsciiStr::from_str(case).unwrap()),
                case
            );
        }
    }
}
