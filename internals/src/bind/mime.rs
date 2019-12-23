use std::borrow::Cow;

use grammar::is_token_char;
use percent_encoding::{percent_encode, EncodeSet};
use soft_ascii_string::{SoftAsciiStr, SoftAsciiString};

#[derive(Debug, Eq, PartialEq, Clone, Copy, Hash)]
struct MimeParamEncodingSet;
impl EncodeSet for MimeParamEncodingSet {
    fn contains(&self, byte: u8) -> bool {
        //if it is in the encoding set we need to encode it
        //which we need to to if it is _not_ a token char
        !is_token_char(byte as char)
    }
}

/// percent encodes a byte sequence so that it can be used
/// in a RFC 2231 conform encoded mime header parameter
pub fn percent_encode_param_value<'a, R>(input: &'a R) -> Cow<'a, SoftAsciiStr>
where
    R: ?Sized + AsRef<[u8]>,
{
    let cow: Cow<'a, str> = percent_encode(input.as_ref(), MimeParamEncodingSet).into();
    match cow {
        Cow::Owned(o) =>
        //SAFE: MimeParamEncodingSet makes all non-us-ascii bytes encoded AND
        // percent_encoding::percent_encode always only produces ascii anyway
        {
            Cow::Owned(SoftAsciiString::from_unchecked(o))
        }
        Cow::Borrowed(b) => Cow::Borrowed(SoftAsciiStr::from_unchecked(b)),
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::borrow::Cow;

    #[test]
    fn encode_simple() {
        let input = "this is tüxt";
        let res = percent_encode_param_value(input);
        assert_eq!("this%20is%20t%C3%BCxt", res.as_str());
    }

    #[test]
    fn no_encode_no_alloc() {
        let input = "full_valid";
        let res = percent_encode_param_value(input);
        assert_eq!(res, Cow::Borrowed(input));
    }
}
