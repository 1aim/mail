use soft_ascii_string::SoftAsciiChar;

use error::ComponentCreationError;
use internals::encoder::{EncodableInHeader, EncodingWriter};
use internals::error::EncodingError;
use {HeaderTryFrom, HeaderTryInto};

use super::Email;
use super::Phrase;

pub struct NoDisplayName;

#[derive(Debug, Hash, Eq, PartialEq, Clone)]
pub struct Mailbox {
    pub display_name: Option<Phrase>,
    pub email: Email,
}

impl Mailbox {
    pub fn auto_gen_name<F>(&mut self, default_fn: F) -> Result<(), ComponentCreationError>
    where
        F: FnOnce(&Email) -> Result<Option<Phrase>, ComponentCreationError>,
    {
        if self.display_name.is_none() {
            let default_name = default_fn(&self.email)?;
            self.display_name = default_name;
        }
        Ok(())
    }

    pub fn with_default_name<F>(mut self, default_fn: F) -> Result<Mailbox, ComponentCreationError>
    where
        F: FnOnce(&Email) -> Result<Option<Phrase>, ComponentCreationError>,
    {
        self.auto_gen_name(default_fn)?;
        Ok(self)
    }
}

impl From<Email> for Mailbox {
    fn from(email: Email) -> Self {
        Mailbox {
            email,
            display_name: None,
        }
    }
}

impl From<(Option<Phrase>, Email)> for Mailbox {
    fn from(pair: (Option<Phrase>, Email)) -> Self {
        let (display_name, email) = pair;
        Mailbox {
            display_name,
            email,
        }
    }
}

impl<E> HeaderTryFrom<E> for Mailbox
where
    E: HeaderTryInto<Email>,
{
    fn try_from(email: E) -> Result<Self, ComponentCreationError> {
        Ok(Mailbox::from(email.try_into()?))
    }
}

impl<E> HeaderTryFrom<(NoDisplayName, E)> for Mailbox
where
    E: HeaderTryInto<Email>,
{
    fn try_from(pair: (NoDisplayName, E)) -> Result<Self, ComponentCreationError> {
        let email = pair.1.try_into()?;
        Ok(Mailbox {
            display_name: None,
            email,
        })
    }
}
impl<P, E> HeaderTryFrom<(Option<P>, E)> for Mailbox
where
    P: HeaderTryInto<Phrase>,
    E: HeaderTryInto<Email>,
{
    fn try_from(pair: (Option<P>, E)) -> Result<Self, ComponentCreationError> {
        let display_name = if let Some(dn) = pair.0 {
            Some(dn.try_into()?)
        } else {
            None
        };
        let email = pair.1.try_into()?;
        Ok(Mailbox {
            display_name,
            email,
        })
    }
}

impl<P, E> HeaderTryFrom<(P, E)> for Mailbox
where
    P: HeaderTryInto<Phrase>,
    E: HeaderTryInto<Email>,
{
    fn try_from(pair: (P, E)) -> Result<Self, ComponentCreationError> {
        let display_name = Some(pair.0.try_into()?);
        let email = pair.1.try_into()?;
        Ok(Mailbox {
            display_name,
            email,
        })
    }
}

impl EncodableInHeader for Mailbox {
    fn encode(&self, handle: &mut EncodingWriter) -> Result<(), EncodingError> {
        if let Some(display_name) = self.display_name.as_ref() {
            display_name.encode(handle)?;
            handle.write_fws();
        }
        //for now this always uses the "<user@do.main>" form even if no display-name is given
        handle.write_char(SoftAsciiChar::from_unchecked('<'))?;
        self.email.encode(handle)?;
        handle.write_char(SoftAsciiChar::from_unchecked('>'))?;
        Ok(())
    }

    fn boxed_clone(&self) -> Box<EncodableInHeader> {
        Box::new(self.clone())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use header_components::{Email, Phrase};

    ec_test! { email_only, {
        let email = Email::try_from( "affen@haus" )?;
        Mailbox::from(email)
    } => ascii => [
        Text "<",
        MarkFWS,
        Text "affen",
        MarkFWS,
        Text "@",
        MarkFWS,
        Text "haus",
        MarkFWS,
        Text ">"
    ]}

    ec_test! { with_display_text, {
        Mailbox {
            display_name: Some( Phrase::try_from( "ay ya" ).unwrap() ),
            email: Email::try_from( "affen@haus" ).unwrap(),
        }
    } => ascii => [
        Text "ay",
        MarkFWS,
        Text " ya",
        MarkFWS,
        Text " <",
        MarkFWS,
        Text "affen",
        MarkFWS,
        Text "@",
        MarkFWS,
        Text "haus",
        MarkFWS,
        Text ">"
    ]}

    mod with_default_name {
        use super::*;

        #[test]
        fn does_nothing_if_display_name_is_set() {
            let mailbox = Mailbox {
                display_name: Some(Phrase::try_from("ay ya").unwrap()),
                email: Email::try_from("ab@cd").unwrap(),
            };
            let mailbox2 = mailbox.clone();
            let mailbox = mailbox.with_default_name(|_| Ok(None)).unwrap();
            assert_eq!(mailbox, mailbox2);
        }

        #[test]
        fn generates_a_display_name_if_needed() {
            let mailbox = Mailbox {
                display_name: None,
                email: Email::try_from("ab@cd").unwrap(),
            };
            let mailbox = mailbox
                .with_default_name(|email| {
                    assert_eq!(email, &Email::try_from("ab@cd").unwrap());
                    Ok(Some(Phrase::try_from("ay ya")?))
                })
                .unwrap();

            assert_eq!(
                mailbox,
                Mailbox {
                    display_name: Some(Phrase::try_from("ay ya").unwrap()),
                    email: Email::try_from("ab@cd").unwrap(),
                }
            );
        }

        #[test]
        fn can_decide_to_not_generate_a_name() {
            let mailbox = Mailbox {
                display_name: None,
                email: Email::try_from("ab@cd").unwrap(),
            };
            let new_mailbox = mailbox.clone().with_default_name(|_| Ok(None)).unwrap();
            assert_eq!(mailbox, new_mailbox);
        }

        #[test]
        fn forward_errors() {
            let mailbox = Mailbox {
                display_name: None,
                email: Email::try_from("ab@cd").unwrap(),
            };
            let result = mailbox
                .clone()
                .with_default_name(|_| Err(ComponentCreationError::new("DisplayName")));
            let err = assert_err!(result);
            assert_eq!(err.to_string(), "creating component DisplayName failed");
        }
    }
}
