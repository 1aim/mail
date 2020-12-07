use soft_ascii_string::{SoftAsciiChar, SoftAsciiStr};

use super::{base64, quoted_printable};

mod impls;
pub use self::impls::*;

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub enum EncodedWordEncoding {
    Base64,
    QuotedPrintable,
}

impl EncodedWordEncoding {
    /// returns the acronym for the given encoding
    /// used in a encoded word
    pub fn acronym(&self) -> &'static SoftAsciiStr {
        use self::EncodedWordEncoding::*;
        match *self {
            Base64 => SoftAsciiStr::from_unchecked("B"),
            QuotedPrintable => SoftAsciiStr::from_unchecked("Q"),
        }
    }

    /// encodes a given utf8 string
    ///
    /// either `self::quoted_printable::encoded_word_encode`
    /// or `self::base64::encoded_word_encode_utf8` is used
    /// depending on which value `self` is.
    ///
    /// As both algorithm need to know about code point boundaries
    /// only encoding utf8 is supported for now
    ///
    pub fn encode<R, O>(&self, input: R, out: &mut O)
    where
        R: AsRef<str>,
        O: EncodedWordWriter,
    {
        use self::EncodedWordEncoding::*;
        let input: &str = input.as_ref();
        match *self {
            Base64 => base64::encoded_word_encode(input, out),
            QuotedPrintable => quoted_printable::encoded_word_encode_utf8(input, out),
        }
    }
}

pub trait EncodedWordWriter {
    fn write_char(&mut self, ch: SoftAsciiChar);
    fn write_charset(&mut self);
    fn encoding(&self) -> EncodedWordEncoding;
    fn write_ecw_seperator(&mut self);

    /// Returns the maximal length of the paylod/encoded data
    ///
    /// Any number of calls to methods on in trait in any way
    /// should never be able to change the returned value.
    /// Only changing e.g. the charset or encoding should be
    /// able to change what `max_paylod_len` returns.
    fn max_payload_len(&self) -> usize;

    fn write_ecw_start(&mut self) {
        let qm = SoftAsciiChar::from_unchecked('?');
        self.write_char(SoftAsciiChar::from_unchecked('='));
        self.write_char(qm);
        self.write_charset();
        self.write_char(qm);
        let acronym = self.encoding().acronym();
        self.write_str(acronym);
        self.write_char(qm);
    }

    fn write_ecw_end(&mut self) {
        self.write_char(SoftAsciiChar::from_unchecked('?'));
        self.write_char(SoftAsciiChar::from_unchecked('='));
    }

    fn start_next_encoded_word(&mut self) {
        self.write_ecw_end();
        self.write_ecw_seperator();
        self.write_ecw_start();
    }

    fn write_str(&mut self, s: &SoftAsciiStr) {
        for ch in s.chars() {
            self.write_char(ch)
        }
    }
}
