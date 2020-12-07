use std::fmt::{self, Display};
use std::ops::{Deref, DerefMut};

use failure::Fail;
use soft_ascii_string::SoftAsciiChar;

use data::Input;
use error::ComponentCreationError;
use internals::bind::encoded_word::{EncodedWordEncoding, WriterWrapper};
use internals::encoder::{EncodableInHeader, EncodingWriter};
use internals::error::{EncodingError, EncodingErrorKind};
use internals::grammar::is_vchar;
use {HeaderTryFrom, HeaderTryInto};

use super::utils::text_partition::{partition, Partition};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Unstructured {
    //FEATUR_TODO(non_utf8_input): split into parts each possibke having their own encoding
    text: Input,
}

impl Display for Unstructured {
    fn fmt(&self, fter: &mut fmt::Formatter) -> fmt::Result {
        fter.write_str(self.as_str())
    }
}

impl Deref for Unstructured {
    type Target = Input;

    fn deref(&self) -> &Self::Target {
        &self.text
    }
}

impl DerefMut for Unstructured {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.text
    }
}

impl<T> HeaderTryFrom<T> for Unstructured
where
    T: HeaderTryInto<Input>,
{
    fn try_from(text: T) -> Result<Self, ComponentCreationError> {
        let text = text.try_into()?;
        Ok(Unstructured { text })
    }
}

impl EncodableInHeader for Unstructured {
    fn encode(&self, handle: &mut EncodingWriter) -> Result<(), EncodingError> {
        let text: &str = &*self.text;
        if text.len() == 0 {
            return Ok(());
        }

        let partitions = partition(text).map_err(|err| {
            EncodingError::from(err.context(EncodingErrorKind::Malformed)).with_str_context(text)
        })?;

        for block in partitions.into_iter() {
            match block {
                Partition::VCHAR(data) => {
                    let mail_type = handle.mail_type();
                    handle
                        .write_if(data, |s| s.chars().all(|ch| is_vchar(ch, mail_type)))
                        .handle_condition_failure(|handle| {
                            let encoding = EncodedWordEncoding::QuotedPrintable;
                            let mut writer = WriterWrapper::new(encoding, handle);
                            encoding.encode(data, &mut writer);
                            Ok(())
                        })?;
                }
                Partition::SPACE(data) => {
                    let mut had_fws = false;
                    for ch in data.chars() {
                        if ch == '\r' || ch == '\n' {
                            continue;
                        } else if !had_fws {
                            handle.mark_fws_pos();
                            had_fws = true;
                        }
                        handle.write_char(SoftAsciiChar::from_unchecked(ch))?;
                    }
                    if !had_fws {
                        // currently this can only happen if data only consists of '\r','\n'
                        // which we strip which in turn would remove the spacing completely
                        //NOTE: space has to be at last one horizontal-white-space
                        // (required by the possibility of VCHAR partitions being
                        //  encoded words)
                        handle.write_fws();
                    }
                }
            }
        }
        Ok(())
    }

    fn boxed_clone(&self) -> Box<EncodableInHeader> {
        Box::new(self.clone())
    }
}

#[cfg(test)]
mod test {

    use super::*;

    ec_test! { simple_encoding, {
        Unstructured::try_from( "this simple case" )?
    } => ascii => [
        Text "this",
        MarkFWS,
        Text " simple",
        MarkFWS,
        Text " case"
    ]}

    ec_test! { simple_utf8,  {
         Unstructured::try_from( "thüs sümple case" )?
    } => utf8 => [
        Text "thüs",
        MarkFWS,
        Text " sümple",
        MarkFWS,
        Text " case"
    ]}

    ec_test! { encoded_words,  {
         Unstructured::try_from( "↑ ↓ ←→ bA" )?
    } => ascii => [
        Text "=?utf8?Q?=E2=86=91?=",
        MarkFWS,
        Text " =?utf8?Q?=E2=86=93?=",
        MarkFWS,
        Text " =?utf8?Q?=E2=86=90=E2=86=92?=",
        MarkFWS,
        Text " bA"
    ]}

    ec_test! { eats_cr_lf, {
        Unstructured::try_from( "a \rb\n c\r\n " )?
    } => ascii => [
        Text "a",
        MarkFWS,
        Text " b",
        MarkFWS,
        Text " c",
        MarkFWS,
        Text " "
    ]}

    ec_test! { at_last_one_fws, {
        Unstructured::try_from( "a\rb\nc\r\n" )?
    } => ascii => [
        Text "a",
        MarkFWS,
        Text " b",
        MarkFWS,
        Text " c",
        MarkFWS,
        Text " "
    ]}

    ec_test! { kinda_keeps_wsp, {
        Unstructured::try_from("\t\ta  b \t")?
    } => ascii => [
        MarkFWS,
        Text "\t\ta",
        MarkFWS,
        Text "  b",
        MarkFWS,
        Text " \t"
    ]}

    ec_test! { wsp_only_phrase, {
        Unstructured::try_from( " \t " )?
    } => ascii => [
        MarkFWS,
        Text " \t "
    ]}

    ec_test! { long_mixed_input, {
        Unstructured::try_from("Subject: …. AAAAAAAAAAAAAAAAAAA….. AA…")?
    } => ascii => [
        Text "Subject:",
        MarkFWS,
        Text " =?utf8?Q?=E2=80=A6=2E?=",
        MarkFWS,
        Text " =?utf8?Q?AAAAAAAAAAAAAAAAAAA=E2=80=A6=2E=2E?=",
        MarkFWS,
        Text " =?utf8?Q?AA=E2=80=A6?="
    ]}
}
