use quoted_string;

use data::Input;
use error::ComponentCreationError;
use internals::bind::encoded_word::{EncodedWordEncoding, WriterWrapper};
use internals::bind::quoted_string::{InternationalizedMailQsSpec, MailQsSpec};
use internals::encoder::{EncodableInHeader, EncodingWriter};
use internals::error::{EncodingError, EncodingErrorKind};
use internals::grammar::encoded_word::EncodedWordContext;
use internals::grammar::is_atext;
use {HeaderTryFrom, HeaderTryInto};

use super::CFWS;

/// A ward as in the mail grammar (RFC 5322).
///
/// **Warning: This is likely to change in the future before the 1.0 release**.
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct Word {
    pub left_padding: Option<CFWS>,
    pub input: Input,
    pub right_padding: Option<CFWS>,
}

impl<T> HeaderTryFrom<T> for Word
where
    T: HeaderTryInto<Input>,
{
    fn try_from(input: T) -> Result<Self, ComponentCreationError> {
        //TODO there should be a better way, I think I take the grammar to literal here
        // could not any WSP be a potential FWSP, do we really need this kind of fine gained
        // control, it feels kind of useless??
        let input = input.try_into()?;
        //FEATURE_TODO(fail_fast): check if input contains a CTL char,
        //  which is/>>should<< always be an error (through in the standard you could but should
        //  not have them in encoded words)
        Ok(Word {
            left_padding: None,
            input,
            right_padding: None,
        })
    }
}

impl Word {
    pub fn pad_left(&mut self, padding: CFWS) {
        self.left_padding = Some(padding)
    }

    pub fn pad_right(&mut self, padding: CFWS) {
        self.right_padding = Some(padding)
    }
}

/// As word has to be differently encoded, depending on the context it
/// appears in it cannot implement EncodableInHeader, instead we have
/// a function which can be used by type containing it which (should)
/// implement EncodableInHeader
///
/// If `ecw_ctx` is `None` the word can not be encoded as a "encoded-word",
/// if it is `Some(context)` the `context` represents in which context the
/// word does appear, which changes some properties of the encoded word.
///
/// NOTE: != encoded-word, through it might create an encoded-word
pub fn do_encode_word<'a, 'b: 'a>(
    word: &'a Word,
    handle: &'a mut EncodingWriter<'b>,
    ecw_ctx: Option<EncodedWordContext>,
) -> Result<(), EncodingError> {
    if let Some(pad) = word.left_padding.as_ref() {
        pad.encode(handle)?;
    }

    let input: &str = &*word.input;
    let mail_type = handle.mail_type();
    handle
        .write_if(input, |input| {
            (!input.contains("=?")) && input.chars().all(|ch| is_atext(ch, mail_type))
        })
        .handle_condition_failure(|handle| {
            if let Some(_ecw_ctx) = ecw_ctx {
                //FIXME actually use the EncodedWordContext
                let encoding = EncodedWordEncoding::QuotedPrintable;
                let mut writer = WriterWrapper::new(encoding, handle);
                encoding.encode(input, &mut writer);
                Ok(())
            } else {
                let mail_type = handle.mail_type();
                let res = if mail_type.is_internationalized() {
                    //spec needed is mime internationlized
                    quoted_string::quote::<InternationalizedMailQsSpec>(input)
                } else {
                    //spec is mime
                    quoted_string::quote::<MailQsSpec>(input)
                };
                let quoted = res.map_err(|_err| {
                    EncodingError::from(EncodingErrorKind::Malformed).with_str_context(input)
                })?;
                handle.write_str_unchecked(&*quoted)
            }
        })?;

    if let Some(pad) = word.right_padding.as_ref() {
        pad.encode(handle)?;
    }
    Ok(())
}

#[cfg(test)]
mod test {
    use std::mem;

    use internals::encoder::simplify_trace_tokens;
    use internals::encoder::EncodingBuffer;
    use internals::encoder::TraceToken::*;
    use internals::MailType;

    use super::super::FWS;
    use super::*;

    ec_test! {encode_pseudo_encoded_words, {
        let word = Word::try_from( "=?" )?;
        enc_closure!(move |handle: &mut EncodingWriter| {
            do_encode_word( &word, handle, Some( EncodedWordContext::Text ) )
        })
    } => ascii => [
        Text "=?utf8?Q?=3D=3F?="
    ]}

    ec_test! {encode_word, {
        let word = Word::try_from( "a↑b" )?;
        enc_closure!(move |handle: &mut EncodingWriter| {
            do_encode_word( &word, handle, Some( EncodedWordContext::Text ) )
        })
    } => ascii => [
        Text "=?utf8?Q?a=E2=86=91b?="
    ]}

    #[test]
    fn encode_fails() {
        let mut encoder = EncodingBuffer::new(MailType::Ascii);
        let mut handle = encoder.writer();
        let word = Word::try_from("a↑b").unwrap();
        assert_err!(do_encode_word(&word, &mut handle, None));
        handle.undo_header();
    }

    ec_test! {quoted_fallback, {
        let word = Word::try_from( "a\"b" )?;
        enc_closure!(move |handle: &mut EncodingWriter| {
            do_encode_word( &word, handle, None )
        })
    } => ascii => [
        Text r#""a\"b""#
    ]}

    #[test]
    fn encode_word_padding() {
        let words = &[
            (
                Word {
                    left_padding: None,
                    input: "abc".into(),
                    right_padding: None,
                },
                vec![Text("abc".into())],
            ),
            (
                Word {
                    left_padding: Some(CFWS::SingleFws(FWS)),
                    input: "abc".into(),
                    right_padding: None,
                },
                vec![MarkFWS, Text(" abc".into())],
            ),
            (
                Word {
                    left_padding: Some(CFWS::SingleFws(FWS)),
                    input: "abc".into(),
                    right_padding: Some(CFWS::SingleFws(FWS)),
                },
                vec![MarkFWS, Text(" abc".into()), MarkFWS, Text(" ".into())],
            ),
            (
                Word {
                    left_padding: None,
                    input: "abc".into(),
                    right_padding: Some(CFWS::SingleFws(FWS)),
                },
                vec![Text("abc".into()), MarkFWS, Text(" ".into())],
            ),
        ];

        for &(ref word, ref expection) in words.iter() {
            let mut encoder = EncodingBuffer::new(MailType::Ascii);
            {
                let mut handle = encoder.writer();
                do_encode_word(word, &mut handle, None).unwrap();
                mem::forget(handle);
            }
            assert_eq!(&simplify_trace_tokens(encoder.trace.into_iter()), expection);
        }
    }
}
