use std::fmt::{self, Display};
use std::result::Result as StdResult;

use soft_ascii_string::SoftAsciiString;

use error::ComponentCreationError;
use HeaderTryFrom;

use super::inner_item::{InnerAscii, InnerUtf8};

/// a Input is similar to Item a container data container used in different
/// context's with different restrictions, but different to an Item it
/// might contain characters which require encoding (e.g. encoded words)
/// to represent them
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct Input(pub InnerUtf8);

impl Input {
    pub fn into_shared(self) -> Self {
        Input(self.0.into_shared())
    }

    pub fn into_ascii_item(self) -> StdResult<InnerAscii, Input> {
        match self {
            Input(InnerUtf8::Owned(string)) => match SoftAsciiString::from_string(string) {
                Ok(asciied) => Ok(InnerAscii::Owned(asciied)),
                Err(err) => Err(Input(InnerUtf8::Owned(err.into_source()))),
            },
            Input(InnerUtf8::Shared(shared)) => {
                if shared.is_ascii() {
                    Ok(InnerAscii::Owned(SoftAsciiString::from_unchecked(&*shared)))
                } else {
                    Err(Input(InnerUtf8::Shared(shared)))
                }
            }
        }
    }

    pub fn into_ascii_item_unchecked(self) -> InnerAscii {
        match self {
            Input(InnerUtf8::Owned(string)) => {
                InnerAscii::Owned(SoftAsciiString::from_unchecked(string))
            }
            Input(InnerUtf8::Shared(shared)) => {
                InnerAscii::Owned(SoftAsciiString::from_unchecked(&*shared))
            }
        }
    }

    pub fn into_utf8_item(self) -> InnerUtf8 {
        self.0
    }
}

impl<'a> From<&'a str> for Input {
    fn from(s: &'a str) -> Self {
        Input(InnerUtf8::Owned(s.into()))
    }
}

impl From<String> for Input {
    fn from(s: String) -> Self {
        Input(InnerUtf8::Owned(s))
    }
}

impl<'a> HeaderTryFrom<&'a str> for Input {
    fn try_from(val: &'a str) -> Result<Self, ComponentCreationError> {
        Ok(val.into())
    }
}
impl HeaderTryFrom<String> for Input {
    fn try_from(val: String) -> Result<Self, ComponentCreationError> {
        Ok(val.into())
    }
}

impl Into<String> for Input {
    fn into(self) -> String {
        self.0.into()
    }
}

impl Display for Input {
    fn fmt(&self, fter: &mut fmt::Formatter) -> fmt::Result {
        fter.write_str(self.as_str())
    }
}

deref0!( +mut Input => InnerUtf8 );

#[cfg(test)]
mod test {
    use owning_ref::OwningRef;
    use std::sync::Arc;

    use super::*;

    #[test]
    fn input_eq() {
        let a = Input(InnerUtf8::Owned("same".into()));
        let b = Input(InnerUtf8::Shared(
            OwningRef::new(Arc::new(String::from("same"))).map(|v| &**v),
        ));
        assert_eq!(a, b);
    }

    #[test]
    fn input_neq() {
        let a = Input(InnerUtf8::Owned("not same".into()));
        let b = Input(InnerUtf8::Shared(
            OwningRef::new(Arc::new(String::from("not at all same"))).map(|v| &**v),
        ));
        assert_ne!(a, b);
    }
}
