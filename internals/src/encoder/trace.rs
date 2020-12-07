/// If it is a test build the EncodingBuffer will
/// have an additional `pub trace` field,
/// which will contain a Vector of `Token`s
/// generated when writing to the string buffer.
///
/// For example when calling `.write_utf8("hy")`
/// following tokens will be added:
/// `[NowUtf8, Text("hy")]`
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum TraceToken {
    MarkFWS,
    CRLF,
    TruncateToCRLF,
    Text(String),
    NowChar,
    NowStr,
    NowAText,
    NowUtf8,
    NowCondText,
    NowUnchecked,
    NewSection,
    End,
    /// used to seperate a header from a body
    ///
    /// be aware that `Section::BodyPaylod` just
    /// contains the Payload, so e.g. headers from
    /// mime bodies or mime multipart body boundaries
    /// still get written into the string buffer
    BlankLine,
    Body,
}

pub fn simplify_trace_tokens<I: IntoIterator<Item = TraceToken>>(inp: I) -> Vec<TraceToken> {
    use self::TraceToken::*;
    use std::mem;
    let iter = inp.into_iter().filter(|t| match *t {
        NowChar | NowStr | NowAText | NowUtf8 | NowUnchecked | NowCondText => false,
        _ => true,
    });

    let mut out = Vec::new();
    let mut textbf = String::new();
    let mut had_text = false;

    for token in iter {
        match token {
            Text(str) => {
                had_text = true;
                textbf.push_str(&*str)
            }
            e => {
                if had_text {
                    let text = mem::replace(&mut textbf, String::new());
                    out.push(Text(text));
                    had_text = false;
                }
                out.push(e);
            }
        }
    }
    if had_text {
        out.push(Text(textbf))
    }
    out
}

#[macro_export]
macro_rules! ec_test {
    ( $(#[$attr:meta])* $name:ident, $inp:block => $mt:tt => [ $($tokens:tt)* ] ) => (

        $(#[$attr])*
        #[test]
        fn $name() {
            #![allow(unused_mut)]
            use $crate::encoder::EncodingBuffer;
            use std::mem;

            let mail_type = {
                let mt_str = stringify!($mt).to_lowercase();
                match mt_str.as_str() {
                    "utf8" |
                    "internationalized"
                        => $crate::MailType::Internationalized,
                    "ascii"
                        =>  $crate::MailType::Ascii,
                    "mime8" |
                    "mime8bit" |
                    "mime8bitenabled"
                        => $crate::MailType::Mime8BitEnabled,
                    other => panic!( "invalid name for mail type: {}", other)
                }
            };

            let mut encoder = EncodingBuffer::new(mail_type);
            {
                //REFACTOR(catch): use catch block once stable
                let component = (|| -> Result<_, $crate::__FError> {
                    let component = $inp;
                    Ok(Box::new(component) as Box<$crate::encoder::EncodableInHeader>)
                })().unwrap();

                let mut handle = encoder.writer();
                component.encode(&mut handle).unwrap();
                // we do not want to finish writing as we might
                // test just parts of headers
                mem::forget(handle);
            }
            let mut expected: Vec<$crate::encoder::TraceToken> = Vec::new();
            ec_test!{ __PRIV_TO_TOKEN_LIST expected $($tokens)* }
            let got = $crate::encoder::simplify_trace_tokens(encoder.trace.into_iter());
            assert_eq!(got, expected)
        }
    );

    (__PRIV_TO_TOKEN_LIST $col:ident Text $e:expr) => (
        $col.push($crate::encoder::TraceToken::Text({$e}.into()));
    );
    (__PRIV_TO_TOKEN_LIST $col:ident $token:ident) => (
        $col.push($crate::encoder::TraceToken::$token);
    );
    (__PRIV_TO_TOKEN_LIST $col:ident Text $e:expr, $($other:tt)*) => ({
        ec_test!{ __PRIV_TO_TOKEN_LIST $col Text $e }
        ec_test!{ __PRIV_TO_TOKEN_LIST $col $($other)* }
    });
    (__PRIV_TO_TOKEN_LIST $col:ident $token:ident, $($other:tt)*) => (
        ec_test!{ __PRIV_TO_TOKEN_LIST $col $token }
        ec_test!{ __PRIV_TO_TOKEN_LIST $col $($other)* }
    );
    (__PRIV_TO_TOKEN_LIST $col:ident ) => ();
    //conflict with nom due to it using a crate exposing compiler_error...
//    (__PRIV_TO_TOKEN_LIST $col:ident $($other:tt)*) => (
//        compiler_error!( concat!(
//            "syntax error in token list: ", stringify!($($other:tt)*)
//        ))
//    )
}

#[cfg(test)]
mod test {
    use super::super::encodable::EncodeClosure;
    use soft_ascii_string::SoftAsciiStr;

    ec_test! { repreduces_all_tokens,
        {
            EncodeClosure::new(|writer| {
                writer.write_utf8("hy-there")?;
                writer.write_fws();
                writer.write_str(SoftAsciiStr::from_unchecked("tshau-there"))?;
                Ok(())
            })
        } => utf8 => [
            Text "hy-there",
            MarkFWS,
            Text " tshau-there"
        ]
    }
}
