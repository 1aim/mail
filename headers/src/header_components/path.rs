use soft_ascii_string::SoftAsciiChar;

use super::Email;
use error::ComponentCreationError;
use internals::encoder::{EncodableInHeader, EncodingWriter};
use internals::error::EncodingError;
use {HeaderTryFrom, HeaderTryInto};

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct Path(pub Option<Email>);

impl HeaderTryFrom<Option<Email>> for Path {
    fn try_from(opt_mail: Option<Email>) -> Result<Self, ComponentCreationError> {
        Ok(Path(opt_mail))
    }
}

impl<T> HeaderTryFrom<T> for Path
where
    T: HeaderTryInto<Email>,
{
    fn try_from(opt_mail: T) -> Result<Self, ComponentCreationError> {
        Ok(Path(Some(opt_mail.try_into()?)))
    }
}

impl EncodableInHeader for Path {
    fn encode(&self, handle: &mut EncodingWriter) -> Result<(), EncodingError> {
        handle.mark_fws_pos();
        handle.write_char(SoftAsciiChar::from_unchecked('<'))?;
        if let Some(mail) = self.0.as_ref() {
            mail.encode(handle)?;
        }
        handle.write_char(SoftAsciiChar::from_unchecked('>'))?;
        handle.mark_fws_pos();
        Ok(())
    }

    fn boxed_clone(&self) -> Box<EncodableInHeader> {
        Box::new(self.clone())
    }
}
//NOTE for parsing we have to make sure to _require_ '<>' around the email

#[cfg(test)]
mod test {
    use super::*;

    ec_test! {empty_path, {
        Path( None )
    } => ascii => [
        MarkFWS,
        Text "<>",
        MarkFWS
    ]}

    ec_test! {simple_path, {
        Path( Some( Email::try_from( "abc@de.fg" )? ) )
    } => ascii => [
        MarkFWS,
        Text "<",
        MarkFWS,
        Text "abc",
        MarkFWS,
        Text "@",
        MarkFWS,
        Text "de.fg",
        MarkFWS,
        Text ">",
        MarkFWS
    ]}
}
