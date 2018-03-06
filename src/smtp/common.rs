//! This modules contains some of the data types used, like e.g. Response, Request, Envelop etc.
use tokio_smtp::request::Mailbox;
use vec1::Vec1;

use tokio_smtp::request::{Mailbox as SmtpMailbox};

use mail::headers::{Sender, From, To};
use mail::Mail;
use super::error::EnvelopFromMailError;

pub struct EnvelopData {
    from: SmtpMailbox,
    to: Vec1<SmtpMailbox>
}

impl EnvelopData {
    pub fn split(self) -> (SmtpMailbox, Vec1<SmtpMailbox>) {
        let EnvelopData { from, to } = self;
        (from, to)
    }

    pub fn from_mail(mail: &Mail) -> Result<Self, EnvelopFromMailError> {

        let headers = mail.headers();
        let smtp_from =
            if let Some(sender) = headers.get_single(Sender) {
                let sender = sender.map_err(|tpr| EnvelopFromMailError::TypeError(tpr))?;
                //TODO double check with from field
                mailbox2smtp_mailbox(sender);
            } else {
                let from = headers.get_single(From)
                    .ok_or(EnvelopFromMailError::NeitherSenderNorFrom)?
                    .map_err(|tpr| EnvelopFromMailError::TypeError(tpr))?;

                if from.len() > 1 {
                    return Err(EnvelopFromMailError::NoSenderAndMoreThanOneFrom);
                }

                mailbox2smtp_mailbox(from.first());
            };

        let smtp_to =
            if let Some(to) = headers.get_single(To) {
                let to = to.map_err(|tpr| EnvelopFromMailError::TypeError(tpr))?;
                to.mapped_ref(mailbox2smtp_mailbox)
            } else {
                return Err(EnvelopFromMailError::NoToHeaderField);
            };

        //TODO Cc, Bcc

        Ok(EnvelopData {
            from: smtp_from,
            to: smtp_to
        })
    }
}


#[derive(Debug, Clone)]
pub struct MailResponse;


#[derive(Debug, Clone)]
pub struct MailRequest {
    mail: Mail,
    envelop_data: Option<EnvelopData>
}


impl MailRequest {

    pub fn into_mail_with_envelop(self) -> Result<(Mail, EnvelopData), EnvelopFromMailError> {
        let envelop =
            if let Some(envelop) = self.envelop_data { envelop }
            else { EnvelopData::from_mail(&self.mail)? };

        Ok((self.mail, envelop))
    }
}

fn mailbox2smtp_mailbox(mailbox: &Mailbox) -> SmtpMailbox {
    use emailaddress::EmailAddress;
    SmtpMailbox(Some(EmailAddress {
        local: mailbox.email.local_part.as_str().to_owned(),
        domain: mailbox.email.domain.as_str().to_owned(),
    }))
}