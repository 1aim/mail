use quoted_printable as extern_quoted_printable;
use soft_ascii_string::{SoftAsciiChar, SoftAsciiString};

use super::encoded_word::EncodedWordWriter;
use error::{EncodingError, EncodingErrorKind};
use failure::Fail;

/// a quoted printable encoding suitable for content transfer encoding,
/// but _not_ suited for the encoding in encoded words
pub fn normal_encode<A: AsRef<[u8]>>(data: A) -> SoftAsciiString {
    let encoded = extern_quoted_printable::encode_to_str(data);
    SoftAsciiString::from_unchecked(encoded)
}

/// a quoted printable decoding suitable for content transfer encoding
#[inline]
pub fn normal_decode<R: AsRef<[u8]>>(input: R) -> Result<Vec<u8>, EncodingError> {
    //extern_quoted_printable h
    extern_quoted_printable::decode(input.as_ref(), extern_quoted_printable::ParseMode::Strict)
        .map_err(|err| err.context(EncodingErrorKind::Malformed).into())
}

/// a quoted printable decoding suitable for decoding a quoted printable
/// encpded text in encoded words
#[inline(always)]
pub fn encoded_word_decode<R: AsRef<[u8]>>(input: R) -> Result<Vec<u8>, EncodingError> {
    //we can just use the stadard decoding
    normal_decode(input)
}

//FIXME we don't use EncodedWord context here,
// instead we use the most restructive context as a basis,
// making it compatilble with all context, but not nessesary
// the best solution...
/// Simple wrapper around ecoded_word_encode for utf8 strings only
pub fn encoded_word_encode_utf8<'a, O>(word: &str, writer: &mut O)
where
    O: EncodedWordWriter,
{
    let iter = word
        .char_indices()
        .map(|(idx, ch)| &word.as_bytes()[idx..idx + ch.len_utf8()]);
    encoded_word_encode(iter, writer);
}

///
/// Quoted Printable encoding for Encoded Words in MIME-Headers
///
/// Which means:
/// 1. there is a limit to the maximum number of characters
///    - the limit is 75 INCLUDING the `=?charset?encoding?...?=` overhead
///    - as such the line length limit of quoted printable can not be hit,
///      the quoted printable part is at most 67 chars long, e.g. for utf8
///      it is at most 64 chars
/// 2. has to be one token, so no ' ','\t' and neither soft nor hard newlines
/// 3. no '?' character
///
/// The input is a sequence of bytes split up in chunks where
/// a split in multipl encoded words can be done between any
/// two chunks but not in a chunk. Wrt. utf8 a chunk would
/// correspond to a character, e.g. `[65]` for `'a'` and
/// `[0xe2, 0x99, 0xa5]` for a `'♥'`.
///
/// Note that a chunk can with more than 21 byte is not guranteed to
/// work, and can trigger a panic.
///
/// As this has to be safe for usage in all header contexts, additional
/// to the chars required by the standard (i.e. '=') following chars are ALWAYS
/// quoted' ', '\t', '?', '(', ')'. Also '\n','\r' see the note below for more
/// details.
///
///
/// # Panics:
///
/// 1. if the encoded size of a chunk is more than 16 byte, which can
///    happen if a chunk has more than 5 bytes. For comparison utf8 has
///    at most chunks with 4 bytes leading to at most 12 byte buffer usage.
///
/// 2. if max size if >76 as no new line handling is implemented and
///    the max size for the use case can be at most 67 chars
///
/// 3. if a single encoded chunk can not be written as one because of
///    the length limitation AFTER a new encoded word was started.
///
/// # Note:
///   as it has to be a token no new line characters can appear in the output,
///   BUT q-encoding also forbids the encoding of CRLF line breaks in TEXT!
///   bodies, which is mean to not mess up with the limitations to the line
///   length, but they are allowed to appear in non TEXT data, but this
///   function should, but might not be limited to be used with text data,
///   which should but might not be limited to data not containing any new
///   line character. For now any appearance of '\r' or '\n' will be encoded
///   like any other "special" byte, for the future a context might be needed.
///   (Especially as encoded words can contain non-ascii text in which '\r','\n'
///   might be encoded with completely different bytes, but when the RFC speaks of
///   '\r','\n' it normally means the bytes 10/13 independent of the character set,
///   or if they appear in a image, zip-archiev etc. )
pub fn encoded_word_encode<'a, I, O>(input: I, out: &mut O)
where
    I: Iterator<Item = &'a [u8]>,
    O: EncodedWordWriter,
{
    out.write_ecw_start();
    let max_payload_len = out.max_payload_len();
    let mut remaining = max_payload_len;
    //WARN: on remaining being > 67
    let mut buf = [SoftAsciiChar::from_unchecked('X'); 16];

    for chunk in input {
        let mut buf_idx = 0;

        for byte in chunk {
            let byte = *byte;
            match byte {
                // this is the way to go as long as we don't want to behave differently for
                // different context, the COMMENT context allows more chars, and the
                // TEXT context even more
                b'!'
                | b'*'
                | b'+'
                | b'-'
                | b'/'
                | b'_'
                | b'0'...b'9'
                | b'A'...b'Z'
                | b'a'...b'z' => {
                    buf[buf_idx] = SoftAsciiChar::from_unchecked(byte as char);
                    buf_idx += 1;
                }
                _otherwise => {
                    buf[buf_idx] = SoftAsciiChar::from_unchecked('=');
                    buf[buf_idx + 1] = lower_nibble_to_hex(byte >> 4);
                    buf[buf_idx + 2] = lower_nibble_to_hex(byte);
                    buf_idx += 3;
                }
            }
        }
        if buf_idx > remaining {
            out.start_next_encoded_word();
            remaining = max_payload_len;
        }
        if buf_idx > remaining {
            panic!(
                "single character longer then max length ({:?}) of encoded word",
                remaining
            );
        }
        for idx in 0..buf_idx {
            out.write_char(buf[idx])
        }
        remaining -= buf_idx;
    }
    out.write_ecw_end()
}

#[inline]
fn lower_nibble_to_hex(half_byte: u8) -> SoftAsciiChar {
    static CHARS: &[char] = &[
        '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'A', 'B', 'C', 'D', 'E', 'F',
    ];

    SoftAsciiChar::from_unchecked(CHARS[(half_byte & 0x0F) as usize])
}

#[cfg(test)]
mod test {
    use super::super::encoded_word::VecWriter;
    use super::*;
    use bind::encoded_word::EncodedWordEncoding;
    use soft_ascii_string::SoftAsciiStr;

    #[test]
    fn to_hex() {
        let data = &[
            ('0', 0b11110000),
            ('0', 0b0),
            ('7', 0b0111),
            ('7', 0b10111),
            ('F', 0b1111),
        ];
        for &(ch, byte) in data {
            assert_eq!(lower_nibble_to_hex(byte), ch);
        }
    }

    macro_rules! test_ecw_encode {
        ($name:ident, data $data:expr => [$($item:expr),*]) => {
            #[test]
            fn $name() {
                let test_data = $data;
                let mut out = VecWriter::new(
                    SoftAsciiStr::from_unchecked("utf8"),
                    EncodedWordEncoding::QuotedPrintable
                );

                encoded_word_encode_utf8( test_data, &mut out );

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

    test_ecw_encode! { can_be_used_in_comments,
        data "()\"" => [
            "=?utf8?Q?=28=29=22?="
        ]
    }

    test_ecw_encode! { can_be_used_in_phrase,
        data "{}~@#$%^&*()=|\\[]';:." => [
            "=?utf8?Q?=7B=7D=7E=40=23=24=25=5E=26*=28=29=3D=7C=5C=5B=5D=27=3B=3A=2E?="
        ]
    }

    test_ecw_encode! { bad_chars_in_all_contexts,
        data "?= \t\r\n" => [
            "=?utf8?Q?=3F=3D=20=09=0D=0A?="
        ]
    }

    test_ecw_encode! { encode_ascii,
        data  "abcdefghijklmnopqrstuvwxyz \t?=0123456789!@#$%^&*()_+-" => [
             "=?utf8?Q?abcdefghijklmnopqrstuvwxyz=20=09=3F=3D0123456789!=40=23=24=25=5E?=",
             "=?utf8?Q?=26*=28=29_+-?="
        ]
    }

    test_ecw_encode! { how_it_handales_newlines,
        data "\r\n" => [
            "=?utf8?Q?=0D=0A?="
        ]
    }

    test_ecw_encode! { split_into_multiple_ecws,
        data "0123456789012345678901234567890123456789012345678901234567891234newline" => [
            "=?utf8?Q?0123456789012345678901234567890123456789012345678901234567891234?=",
            "=?utf8?Q?newline?="
        ]
    }

    test_ecw_encode! { bigger_chunks,
        data "ランダムテキスト ראַנדאָם טעקסט" => [
            //ランダムテキス
            "=?utf8?Q?=E3=83=A9=E3=83=B3=E3=83=80=E3=83=A0=E3=83=86=E3=82=AD=E3=82=B9?=",
            //ト ראַנדאָם
            "=?utf8?Q?=E3=83=88=20=D7=A8=D7=90=D6=B7=D7=A0=D7=93=D7=90=D6=B8=D7=9D=20?=",
            //טעקסט
            "=?utf8?Q?=D7=98=D7=A2=D7=A7=D7=A1=D7=98?="
        ]
    }

    #[test]
    fn ecw_decode() {
        let pairs = [
            ("=28=29=22", "()\""),
            (
                "=7B=7D=7E=40=23=24=25=5E=26*=28=29=3D=7C=5C=5B=5D=27=3B=3A=2E",
                "{}~@#$%^&*()=|\\[]';:.",
            ),
            ("=3F=3D=20=09=0D=0A", "?= \t\r\n"),
            ("=26*=28=29_+-", "&*()_+-"),
            (
                "abcdefghijklmnopqrstuvwxyz=20=09=3F=3D0123456789!=40=23=24=25=5E",
                "abcdefghijklmnopqrstuvwxyz \t?=0123456789!@#$%^",
            ),
            ("=0D=0A", "\r\n"),
            (
                "=E3=83=A9=E3=83=B3=E3=83=80=E3=83=A0=E3=83=86=E3=82=AD=E3=82=B9",
                "ランダムテキス",
            ),
            (
                "=E3=83=88=20=D7=A8=D7=90=D6=B7=D7=A0=D7=93=D7=90=D6=B8=D7=9D=20",
                "ト ראַנדאָם ",
            ),
            ("=D7=98=D7=A2=D7=A7=D7=A1=D7=98", "טעקסט"),
        ];
        for &(inp, outp) in pairs.iter() {
            let dec = assert_ok!(encoded_word_decode(inp));
            let dec = String::from_utf8(dec).unwrap();
            assert_eq!(outp.as_bytes(), dec.as_bytes());
        }
    }

    #[test]
    fn normal_encode_text() {
        let text = concat!(
            "This is a llllllllllllllllllllllllllllllllllllll00000000000000000000ng test    0123456789qwertyuio\r\n",
            "With many lines\r\n",
            "And utf→→→→8"
        );
        let encoded = normal_encode(text);
        assert_eq!(
            concat!(
                "This is a llllllllllllllllllllllllllllllllllllll00000000000000000000ng test=\r\n",
                "    0123456789qwertyuio\r\n",
                "With many lines\r\n",
                "And utf=E2=86=92=E2=86=92=E2=86=92=E2=86=928"
            ),
            encoded.as_str()
        );
    }

    #[test]
    fn normal_decode_text() {
        let text = concat!(
            "This is a llllllllllllllllllllllllllllllllllllll00000000000000000000ng test=\r\n",
            "    0123456789qwertyuio\r\n",
            "With many lines\r\n",
            "And utf=E2=86=92=E2=86=92=E2=86=92=E2=86=928"
        );
        let encoded = String::from_utf8(normal_decode(text).unwrap()).unwrap();
        assert_eq!(
            concat!(
                "This is a llllllllllllllllllllllllllllllllllllll00000000000000000000ng test    0123456789qwertyuio\r\n",
                "With many lines\r\n",
                "And utf→→→→8"
            ),
            encoded.as_str()
        );
    }
}
