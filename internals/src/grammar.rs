//! This module contains a number of helper functions for writing parsers.
//!
//! Ironically they are also needed when writing mail encoders/generators
//! e.g. for checking if a part need special encoding.
use MailType;

/// ftext as defined by RFC 5322
///
/// which is: printable US-ASCII characters not includign `:`
///  => 0x21-0x39 / 0x3B-0x7E
///  => '!'...'9' / ';'...'~'
///  => <0x7F && != 0x3A
#[inline(always)]
pub fn is_ftext(ch: char) -> bool {
    let bch = ch as u32;
    bch > 32 && bch < 127 && ch != ':'
}

///WS as defined by RFC 5234
#[inline(always)]
pub fn is_ws(ch: char) -> bool {
    // is not limited to ascii ws
    //ch.is_whitespace()
    //WSP            =  SP / HTAB
    ch == ' ' || ch == '\t'
}

/// True if `ch` is `' '`
#[inline(always)]
pub fn is_space(ch: char) -> bool {
    ch == ' '
}

/// True if `ch` is us-ascii (i.e. <128)
#[inline(always)]
pub fn is_ascii(ch: char) -> bool {
    (ch as u32) < 128
}

/// True if `ch` is ascii and "visible"/"printable".
///
/// This is the case for any char in the (decimal)
/// range 33..=126 which is '!'..='~'.
#[inline(always)]
pub fn is_ascii_vchar(ch: char) -> bool {
    let u32_ch = ch as u32;
    32 < u32_ch && u32_ch <= 126
}

/// VCHAR as defined by RFC 5243
///
/// Is true if it's either an us-ascii vchar or
/// an non us-ascii char and the mail type is
/// internationalized.
///
/// This mean that this includes _non printable_
/// characters as long as the mail is internationalized
/// and the character is non us-ascii utf-8.
#[inline(always)]
pub fn is_vchar(ch: char, mt: MailType) -> bool {
    is_ascii_vchar(ch) || (mt == MailType::Internationalized && !is_ascii(ch))
}

//TODO as RFCs
/// can be quoted in a quoted string (internalized) based on RFC ... and RFC ...
#[inline(always)]
pub fn is_quotable(ch: char, tp: MailType) -> bool {
    is_vchar(ch, tp) || is_ws(ch)
}

/// any whitespace (char::is_whitespace)
#[inline(always)]
pub fn is_any_whitespace(ch: char) -> bool {
    ch.is_whitespace()
}

/// ctext as defined by RFC 5322
pub fn is_ctext(ch: char, mt: MailType) -> bool {
    match ch {
        '!'...'\'' | '*'...'[' | ']'...'~' => true,
        // obs-ctext
        _ => mt == MailType::Internationalized && !is_ascii(ch),
    }
}

/// check if a char is a especial (_based on RFC 5322_)
///
/// Note that there is _another_ especial from a different RFC.
pub fn is_special(ch: char) -> bool {
    match ch {
        '(' | ')' | '<' | '>' | '[' | ']' | ':' | ';' | '@' | '\\' | ',' | '.' | '"' => true,
        _ => false,
    }
}

/// check if a char is an tspecial (based on RFC 2045)
pub fn is_tspecial(ch: char) -> bool {
    match ch {
        '(' | ')' | '<' | '>' | '@' | ',' | ';' | ':' | '\\' | '"' | '/' | '[' | ']' | '?'
        | '=' => true,
        _ => false,
    }
}

/// atext as defined by RFC 5322
#[inline(always)]
pub fn is_atext(ch: char, tp: MailType) -> bool {
    is_vchar(ch, tp) && !is_special(ch)
}

/// dtext as defined by RFC 5322
#[inline(always)]
pub fn is_dtext(ch: char, mt: MailType) -> bool {
    match ch as u32 {
        33...90 | 94...126 => true,
        _ => mt == MailType::Internationalized && !is_ascii(ch),
    }
}

/// qtext as defined by RFC 5322
pub fn is_qtext(ch: char, mt: MailType) -> bool {
    match ch {
        //not ' ' [d:32]
        '!' |
        //not '"' [d:34]
        '#'...'[' |
        //not '\\' [d:92]
        ']'...'~' => true,
        _ => mt == MailType::Internationalized && !is_ascii(ch)
    }
}

/// Chack if it is a CTL char (based on RFC 822).
///
/// # Note
/// the standard specifies `'\t'` as a CTL but not `' '`
/// but both `'\t'` and `' '` are LWSP-char i.e. semantically
/// space i.e. _semantically equivalent_.
#[inline(always)]
pub fn is_ctl(ch: char) -> bool {
    (ch as u32) < 32
}

/// Check if a char is an token char (based on RFC 2045).
#[inline(always)]
pub fn is_token_char(ch: char) -> bool {
    is_ascii(ch) && !is_ctl(ch) && !is_tspecial(ch) && ch != ' '
}

//TODO add rfc
/// Check if a char is especial (based on RFC ...).
#[inline(always)]
pub fn is_especial(ch: char) -> bool {
    match ch {
        '(' | ')' | '<' | '>' | '@' | ',' | ';' | ':' | '"' | '/' | '[' | ']' | '?' | '.' | '=' => {
            true
        }
        _ => false,
    }
}

//TODO add rfc
/// Check if a string is an token (based on RFC ...).
pub fn is_token(s: &str) -> bool {
    0 < s.len() && s.chars().all(is_token_char)
}

//
//pub fn is_dot_atom_text( text: &str, mt: MailType ) -> bool {
//    use nom::IResult;
//    use self::parse::recognize_dot_atom_text;
//
//    let res = tuple!( text,
//        call!( recognize_dot_atom_text, mt ),
//        eof!()
//    );
//
//    match res {
//        IResult::Done(_, _) => true,
//        _ => false
//    }
//}

//pub mod parse {
//    use nom::IResult;
//    use super::{ is_atext, MailType };
//
//    pub fn recognize_dot_atom_text( input: &str, mt: MailType ) -> IResult<&str, &str> {
//        recognize!( input, tuple!(
//            take_while1!( call!( is_atext, mt ) ),
//            many0!( tuple!(
//                char!( "." ),
//                take_while1!( call!( is_atext, mt ) )
//            ) )
//        ) )
//    }
//
//}
//TODO this should be some where else I think
// (but it is used by `1. codec`, `2. components` )
/// Grammar parts for encoded words (based on RFC 2047).
pub mod encoded_word {
    use super::{is_ascii_vchar, is_especial};
    use error::{EncodingError, EncodingErrorKind};
    use nom;
    use MailType;

    /// maximal length of an encoded word
    pub const MAX_ECW_LEN: usize = 75;

    /// The syntax overhead from "framing" an encoded word.
    ///
    /// This is the start (1x`=?`) the first and second separator (2x`?`) and the
    /// end (1x`?=`) leading to 6 byte overhead.
    pub const ECW_SEP_OVERHEAD: usize = 6;

    /// Represents the place at which the encoded word appears.
    ///
    /// Depending on the place more or less character have to be
    /// encoded.
    ///
    /// Note: Implementations creating encoded words might use a
    /// stricter context which is compatible with all places to
    /// reduce code complexity.
    #[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
    pub enum EncodedWordContext {
        Phrase,
        Text,
        Comment,
    }

    impl EncodedWordContext {
        /// Returns a (context dependent) validator to check if a char can be represented without encoding.
        fn char_validator(&self) -> fn(char) -> bool {
            use self::EncodedWordContext::*;
            match *self {
                Phrase => valid_char_in_ec_in_phrase,
                Text => is_encoded_word_char,
                Comment => valid_char_in_ec_in_comment,
            }
        }
    }

    /// Returns true if the given word is a encoded word.
    ///
    /// Note that this depends on the context the word appears in and the mail type.
    /// The reason for this is that encoded words tend to be valid text even without
    /// decoding them. But this means if the encoded word has some syntax error (e.g.
    /// missing closing `?=`) it is no longer an encoded word but just some text which
    /// happen to look similar to one.
    pub fn is_encoded_word(word: &str, ctx: EncodedWordContext, mail_type: MailType) -> bool {
        try_parse_encoded_word_parts(word, ctx, mail_type).is_ok()
    }

    /// Tries to parse the given string as an encoded word.
    pub fn try_parse_encoded_word_parts(
        word: &str,
        ctx: EncodedWordContext,
        mail_type: MailType,
    ) -> Result<(&str, &str, &str), EncodingError> {
        let char_validator = ctx.char_validator();
        // Note we could get a possible speed up by making rustc generate
        // a different function for each Context, inlining ALL char tests
        let res = do_parse!(
            word,
            char!('=')
                >> char!('?')
                >> charset: take_while!(is_ew_token_char)
                >> char!('?')
                >> encoding: take_while!(is_ew_token_char)
                >> char!('?')
                >> text: take_while!(char_validator)
                >> char!('?')
                >> char!('=')
                >> eof!()
                >> (charset, encoding, text)
        );

        match res {
            nom::IResult::Done(rest, result) => {
                assert_eq!(rest.len(), 0, "[BUG] used nom::eof!() but rest.len() > 0");
                Ok(result)
            }
            nom::IResult::Incomplete(..) => {
                return Err((EncodingErrorKind::Malformed, mail_type).into());
            }
            nom::IResult::Error(..) => {
                return Err((EncodingErrorKind::Malformed, mail_type).into());
            }
        }
    }

    /// True if the char can appear in an encoded word.
    fn is_encoded_word_char(ch: char) -> bool {
        is_ascii_vchar(ch) && ch != '?'
    }

    /// True if the char can appear in an encoded word appearing in a comment.
    fn valid_char_in_ec_in_comment(ch: char) -> bool {
        is_encoded_word_char(ch) && !(ch == '(' || ch == ')' || ch == '"')
    }

    /// True if the char is valid in an encode word appearing in a phrase.
    fn valid_char_in_ec_in_phrase(ch: char) -> bool {
        match ch {
            '0'...'9' | 'a'...'z' | 'A'...'Z' | '!' | '*' | '+' | '-' | '/' | '=' | '_' => true,
            _ => false,
        }
    }

    /// True if the char is a encoded word token.
    ///
    /// Encoded word tokens are used for the charset and
    /// language part of an encoded word.
    fn is_ew_token_char(ch: char) -> bool {
        is_ascii_vchar(ch) && !is_especial(ch)
    }
}

//TODO shouldn't we use `bind/quoted_string`?
/// True if the given string is a quoted string.
pub fn is_quoted_string(qstr: &str, tp: MailType) -> bool {
    let mut iter = qstr.chars();
    if let Some('"') = iter.next() {
    } else {
        return false;
    }
    let mut next = iter.next();
    while let Some(ch) = next {
        match ch {
            '\\' => {
                if let Some(next_char) = iter.next() {
                    if !(is_vchar(next_char, tp) || is_ws(next_char)) {
                        return false;
                    }
                } else {
                    return false;
                }
            }
            '"' => {
                if iter.next().is_none() {
                    return true;
                } else {
                    return false;
                }
            }
            ch => {
                if !is_qtext(ch, tp) {
                    return false;
                }
            }
        }
        next = iter.next()
    }

    // The only true return if we have a '"' followed by iter.next().is_none()
    return false;
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn _is_ascii_vchar() {
        assert_eq!(false, is_ascii_vchar('\x7f'));
        for bad_char in b'\0'..b' ' {
            if is_ascii_vchar(bad_char as char) {
                panic!("{:?} should not be a VCHAR", bad_char);
            }
        }
        for good_char in b'!'..(b'~' + 1) {
            if !is_ascii_vchar(good_char as char) {
                panic!("{:?} should be a VCHAR", good_char as char);
            }
        }
    }

    #[test]
    fn htap_is_ctl_space_is_not() {
        assert_eq!(true, is_ctl('\t'));
        assert_eq!(false, is_ctl(' '));
    }

    #[test]
    fn is_toke_empty() {
        assert_eq!(false, is_token(""));
    }
}
