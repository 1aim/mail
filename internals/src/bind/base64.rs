use base64 as extern_base64;
use failure::Fail;
use soft_ascii_string::{SoftAsciiChar, SoftAsciiString};

use error::{EncodingError, EncodingErrorKind};
use utils::is_utf8_continuation_byte;

use super::encoded_word::EncodedWordWriter;

const CHARSET: extern_base64::CharacterSet = extern_base64::CharacterSet::Standard;
const NO_LINE_WRAP: extern_base64::LineWrap = extern_base64::LineWrap::NoWrap;
const LINE_WRAP: extern_base64::LineWrap =
    extern_base64::LineWrap::Wrap(78, extern_base64::LineEnding::CRLF);
const USE_PADDING: bool = true;
const ECW_STRIP_WHITESPACE: bool = false;
const NON_ECW_STRIP_WHITESPACE: bool = true;

#[inline]
pub fn normal_encode<R: AsRef<[u8]>>(input: R) -> SoftAsciiString {
    let res = extern_base64::encode_config(
        input.as_ref(),
        extern_base64::Config::new(
            //FIXME: check if line wrap should be used here, I thinks it should
            CHARSET,
            USE_PADDING,
            NON_ECW_STRIP_WHITESPACE,
            LINE_WRAP,
        ),
    );
    SoftAsciiString::from_unchecked(res)
}

#[inline]
pub fn normal_decode<R: AsRef<[u8]>>(input: R) -> Result<Vec<u8>, EncodingError> {
    extern_base64::decode_config(
        input.as_ref(),
        extern_base64::Config::new(CHARSET, USE_PADDING, NON_ECW_STRIP_WHITESPACE, LINE_WRAP),
    )
    .map_err(|err| err.context(EncodingErrorKind::Malformed).into())
}

#[inline(always)]
fn calc_max_input_len(max_output_len: usize) -> usize {
    //NOTE: *3/4 is NOT correct due to the way this
    // relies on non-floting point division
    max_output_len / 4 * 3
}

//NOTE: base64 does not have to care about the EncodedWordContext,
// it is valid under all of them anyway
///
/// # Note
/// for now this only supports utf8/ascii input, as
/// we have to know where we can split
#[inline(always)]
pub fn encoded_word_encode<O, R: AsRef<str>>(input: R, out: &mut O)
where
    O: EncodedWordWriter,
{
    _encoded_word_encode(input.as_ref(), out)
}

fn _encoded_word_encode<O>(input: &str, out: &mut O)
where
    O: EncodedWordWriter,
{
    let config =
        extern_base64::Config::new(CHARSET, USE_PADDING, ECW_STRIP_WHITESPACE, NO_LINE_WRAP);

    debug_assert!(
        USE_PADDING == true,
        "size calculation is tailored for padding"
    );

    let max_output_len = out.max_payload_len();
    let max_input_len = calc_max_input_len(max_output_len);
    let mut rest = input;
    let mut buff = String::with_capacity(max_output_len);

    out.write_ecw_start();

    loop {
        buff.clear();

        // additional bytes in uf8 always start with binary b10xxxxxx
        let rest_len = rest.len();
        let split_idx = if max_input_len >= rest_len {
            rest_len
        } else {
            let mut tmp_split = max_input_len;
            let rest_bytes = rest.as_bytes();

            // the byte at the current index starts with that we are in a
            // position where we can't split and have to move left until
            // the beginning of the utf8
            while is_utf8_continuation_byte(rest_bytes[tmp_split]) {
                //UNDERFLOW_SAFE: if the string is correct (contains valid utf8) this cant undeflow as
                // the first byte cant start with 0b10xxxxxx.
                tmp_split -= 1;
            }
            tmp_split
        };

        let (this, _rest) = rest.split_at(split_idx);
        //very important ;=)
        rest = _rest;

        extern_base64::encode_config_buf(this, config.clone(), &mut buff);
        //FIXME add a write_str method to EncodedWordWriter
        for ch in buff.chars() {
            //SAFE: base64 consist of only ascii chars
            out.write_char(SoftAsciiChar::from_unchecked(ch))
        }

        if rest.len() == 0 {
            break;
        } else {
            out.start_next_encoded_word();
        }
    }
    out.write_ecw_end();
}

#[inline(always)]
pub fn encoded_word_decode<R: AsRef<[u8]>>(input: R) -> Result<Vec<u8>, EncodingError> {
    extern_base64::decode_config(
        input.as_ref(),
        extern_base64::Config::new(CHARSET, USE_PADDING, ECW_STRIP_WHITESPACE, NO_LINE_WRAP),
    )
    .map_err(|err| err.context(EncodingErrorKind::Malformed).into())
}

#[cfg(test)]
mod test {
    use super::*;
    use bind::encoded_word::{EncodedWordEncoding, VecWriter};
    use soft_ascii_string::SoftAsciiStr;

    #[test]
    fn encoding_uses_line_wrap() {
        let input = concat!(
            "0123456789",
            "0123456789",
            "0123456789",
            "0123456789",
            "0123456789",
            "0123456789",
        );

        let res = normal_encode(input);

        assert_eq!(
            res.as_str(),
            "MDEyMzQ1Njc4OTAxMjM0NTY3ODkwMTIzNDU2Nzg5MDEyMzQ1Njc4OTAxMjM0NTY3ODkwMTIzNDU2Nz\r\ng5"
        );

        let dec = normal_decode(res).unwrap();

        assert_eq!(dec, input.as_bytes());
    }

    #[test]
    fn calc_max_input_len_from_max_output_len() {
        assert!(USE_PADDING, "algorithm is specific to the usage of padding");
        assert_eq!(45, calc_max_input_len(60));
        assert_eq!(45, calc_max_input_len(61));
        assert_eq!(45, calc_max_input_len(62));
        assert_eq!(45, calc_max_input_len(63));
        assert_eq!(48, calc_max_input_len(64));
    }

    #[test]
    fn encode_decode_normal() {
        let pairs: &[(&str, &[u8])] = &[
            (
                "this is some\r\nlong\r\ntest.",
                b"dGhpcyBpcyBzb21lDQpsb25nDQp0ZXN0Lg==",
            ),
            ("", b""),
        ];
        for &(raw, encoded) in pairs.iter() {
            assert_eq!(normal_encode(raw).as_bytes(), encoded);
            assert_eq!(assert_ok!(normal_decode(encoded)), raw.as_bytes())
        }
    }

    macro_rules! test_ecw_encode {
        ($name:ident, data $data:expr => [$($item:expr),*]) => {
            #[test]
            fn $name() {
                let test_data = $data;
                let mut out = VecWriter::new(
                    SoftAsciiStr::from_unchecked("utf8"),
                    EncodedWordEncoding::Base64
                );

                encoded_word_encode( test_data, &mut out );

                let expected = &[
                    $($item),*
                ];

                let iter = expected.iter()
                    .zip( out.data().iter().map(|x|x.as_str()) )
                    .enumerate();

                for ( idx, (expected, got) ) in iter {
                    if  *expected != got {
                        panic!( " item nr {}: {:?} != {:?} ", idx, expected, got );
                    }
                }

                let e_len = expected.len();
                let g_len = out.data().len();
                if e_len > g_len {
                    panic!( "expected following additional items: {:?}", &expected[g_len..e_len])
                }
                if e_len < g_len {
                    panic!( "got following additional items: {:?}", &out.data()[e_len..g_len])
                }
            }
        };
    }

    test_ecw_encode! { ecw_simple,
        data "()\"" => [
            "=?utf8?B?KCki?="
        ]
    }

    test_ecw_encode! { ecw_simple_max_len,
        data "012345678901234567890123456789012345678944448888" => [
            "=?utf8?B?MDEyMzQ1Njc4OTAxMjM0NTY3ODkwMTIzNDU2Nzg5MDEyMzQ1Njc4OTQ0NDQ4ODg4?="
        ]
    }

    test_ecw_encode! { multiple_ecws,
        data "012345678901234567890123456789012345678944448888NEWWORD" => [
            "=?utf8?B?MDEyMzQ1Njc4OTAxMjM0NTY3ODkwMTIzNDU2Nzg5MDEyMzQ1Njc4OTQ0NDQ4ODg4?=",
            "=?utf8?B?TkVXV09SRA==?="
        ]
    }

    test_ecw_encode! { ecw_end_in_multibyte_codepoint,
        data "01234567890123456789012345678901234567894444888↓" => [
            "=?utf8?B?MDEyMzQ1Njc4OTAxMjM0NTY3ODkwMTIzNDU2Nzg5MDEyMzQ1Njc4OTQ0NDQ4ODg=?=",
            "=?utf8?B?4oaT?="
        ]
    }

    #[test]
    fn decode_encoded_word() {
        assert_eq!(
            assert_ok!(encoded_word_decode("dGhpc19jcmF6eV9lbmNvZGVkX3dvcmQ=")),
            b"this_crazy_encoded_word"
        );
    }
}
