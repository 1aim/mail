use soft_ascii_string::SoftAsciiStr;

use internals::encoder::{EncodableInHeader, EncodingWriter};
use internals::error::EncodingError;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// The TransferEnecoding header component mainly used by the ContentTransferEncodingHeader.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum TransferEncoding {
    #[cfg_attr(feature = "serde", serde(rename = "7bit"))]
    _7Bit,
    #[cfg_attr(feature = "serde", serde(rename = "8bit"))]
    _8Bit,
    #[cfg_attr(feature = "serde", serde(rename = "binary"))]
    Binary,
    #[cfg_attr(feature = "serde", serde(rename = "quoted-printable"))]
    QuotedPrintable,
    #[cfg_attr(feature = "serde", serde(rename = "base64"))]
    Base64,
}

impl TransferEncoding {
    pub fn repr(&self) -> &SoftAsciiStr {
        use self::TransferEncoding::*;
        match *self {
            _7Bit => SoftAsciiStr::from_unchecked("7bit"),
            _8Bit => SoftAsciiStr::from_unchecked("8bit"),
            Binary => SoftAsciiStr::from_unchecked("binary"),
            QuotedPrintable => SoftAsciiStr::from_unchecked("quoted-printable"),
            Base64 => SoftAsciiStr::from_unchecked("base64"),
        }
    }
}

impl EncodableInHeader for TransferEncoding {
    fn encode(&self, handle: &mut EncodingWriter) -> Result<(), EncodingError> {
        handle.write_str(self.repr())?;
        Ok(())
    }

    fn boxed_clone(&self) -> Box<EncodableInHeader> {
        Box::new(self.clone())
    }
}

#[cfg(test)]
mod test {
    use super::TransferEncoding;

    ec_test! {_7bit, {
        TransferEncoding::_7Bit
    } => ascii => [
        Text "7bit"
    ]}

    ec_test! {_8bit, {
        TransferEncoding::_8Bit
    } => ascii => [
        Text "8bit"
    ]}

    ec_test! {binary, {
        TransferEncoding::Binary
    } => ascii => [
        Text "binary"
    ]}

    ec_test! {base64, {
        TransferEncoding::Base64
    } => ascii => [
        Text "base64"
    ]}

    ec_test! {quoted_printable, {
        TransferEncoding::QuotedPrintable
    } => ascii => [
        Text "quoted-printable"
    ]}

    #[cfg(feature = "serde")]
    mod serde {
        use super::TransferEncoding;
        use serde_test::{assert_tokens, Token};

        macro_rules! serde_token_tests {
            ($([$lname:ident, $hname:ident, $s:tt]),*) => ($(
                #[test]
                fn $lname() {
                    assert_tokens(&TransferEncoding::$hname, &[
                        Token::UnitVariant {
                            name: "TransferEncoding",
                            variant: $s
                        }
                    ])
                }
            )*);
        }

        serde_token_tests! {
            [_7bit, _7Bit, "7bit"],
            [_8bit, _8Bit, "8bit"],
            [binary, Binary, "binary"],
            [quoted_printable, QuotedPrintable, "quoted-printable"],
            [base64, Base64, "base64"]
        }
    }
}
