use std::mem;

use new_tokio_smtp::send_mail::{self as smtp, EnvelopData, MailAddress};

use headers::{
    error::BuildInValidationError,
    header_components::Mailbox,
    headers::{Sender, _From, _To},
};
use mail::{
    error::{MailError, OtherValidationError},
    Mail,
};
use mail_internals::{
    encoder::{EncodableInHeader, EncodingBuffer},
    error::EncodingError,
    MailType,
};

use error::OtherValidationError as AnotherOtherValidationError;

/// This type contains a mail and potentially some envelop data.
///
/// It is needed as in some edge cases the smtp envelop data (i.e.
/// smtp from and smtp recipient) can not be correctly derived
/// from the mail.
///
/// The default usage is to directly turn a `Mail` into a `MailRequest`
/// by either using  `MailRequest::new`, `MailRequest::from` or `Mail::into`.
///
#[derive(Clone, Debug)]
pub struct MailRequest {
    mail: Mail,
    envelop_data: Option<EnvelopData>,
}

impl From<Mail> for MailRequest {
    fn from(mail: Mail) -> Self {
        MailRequest::new(mail)
    }
}

impl MailRequest {
    /// creates a new `MailRequest` from a `Mail` instance
    pub fn new(mail: Mail) -> Self {
        MailRequest {
            mail,
            envelop_data: None,
        }
    }

    /// create a new `MailRequest` and use custom smtp `EnvelopData`
    ///
    /// Note that envelop data comes from `new-tokio-smtp::send_mail` and
    /// is not re-exported so if you happen to run into one of the view
    /// cases where you need to set it manually just import it from
    /// `new-tokio-smtp`.
    pub fn new_with_envelop(mail: Mail, envelop: EnvelopData) -> Self {
        MailRequest {
            mail,
            envelop_data: Some(envelop),
        }
    }

    /// replace the smtp `EnvelopData`
    pub fn override_envelop(&mut self, envelop: EnvelopData) -> Option<EnvelopData> {
        mem::replace(&mut self.envelop_data, Some(envelop))
    }

    pub fn _into_mail_with_envelop(self) -> Result<(Mail, EnvelopData), MailError> {
        let envelop = if let Some(envelop) = self.envelop_data {
            envelop
        } else {
            derive_envelop_data_from_mail(&self.mail)?
        };

        Ok((self.mail, envelop))
    }

    #[cfg(not(feature = "extended-api"))]
    #[inline(always)]
    pub(crate) fn into_mail_with_envelop(self) -> Result<(Mail, EnvelopData), MailError> {
        self._into_mail_with_envelop()
    }

    /// Turns this type into the contained mail an associated envelop data.
    ///
    /// If envelop data was explicitly set it is returned.
    /// If no envelop data was explicitly given it is derived from the
    /// Mail header fields using `derive_envelop_data_from_mail`.
    #[cfg(feature = "extended-api")]
    #[inline(always)]
    pub fn into_mail_with_envelop(self) -> Result<(Mail, EnvelopData), MailError> {
        self._into_mail_with_envelop()
    }
}

fn mailaddress_from_mailbox(mailbox: &Mailbox) -> Result<MailAddress, EncodingError> {
    let email = &mailbox.email;
    let needs_smtputf8 = email.check_if_internationalized();
    let mt = if needs_smtputf8 {
        MailType::Internationalized
    } else {
        MailType::Ascii
    };
    let mut buffer = EncodingBuffer::new(mt);
    {
        let mut writer = buffer.writer();
        email.encode(&mut writer)?;
        writer.commit_partial_header();
    }
    let raw: Vec<u8> = buffer.into();
    let address = String::from_utf8(raw).expect("[BUG] encoding Email produced non utf8 data");
    Ok(MailAddress::new_unchecked(address, needs_smtputf8))
}

/// Generates envelop data based on the given Mail.
///
/// If a sender header is given smtp will use this
/// as smtp from else the single mailbox in from
/// is used as smtp from.
///
/// All `To`'s are used as smtp recipients.
///
/// **`Cc`/`Bcc` is currently no supported/has no
/// special handling**
///
/// # Error
///
/// An error is returned if there is:
///
/// - No From header
/// - No To header
/// - A From header with multiple addresses but no Sender header
///
pub fn derive_envelop_data_from_mail(mail: &Mail) -> Result<smtp::EnvelopData, MailError> {
    let headers = mail.headers();
    let smtp_from = if let Some(sender) = headers.get_single(Sender) {
        let sender = sender?;
        //TODO double check with from field
        mailaddress_from_mailbox(sender)?
    } else {
        let from = headers
            .get_single(_From)
            .ok_or(OtherValidationError::NoFrom)??;

        if from.len() > 1 {
            return Err(BuildInValidationError::MultiMailboxFromWithoutSender.into());
        }

        mailaddress_from_mailbox(from.first())?
    };

    let smtp_to = if let Some(to) = headers.get_single(_To) {
        let to = to?;
        to.try_mapped_ref(mailaddress_from_mailbox)?
    } else {
        return Err(AnotherOtherValidationError::NoTo.into());
    };

    //TODO Cc, Bcc

    Ok(EnvelopData {
        from: Some(smtp_from),
        to: smtp_to,
    })
}

#[cfg(test)]
mod test {

    mod derive_envelop_data_from_mail {
        use super::super::derive_envelop_data_from_mail;
        use headers::headers::{Sender, _From, _To};
        use mail::{test_utils::CTX, Mail, Resource};

        fn mock_resource() -> Resource {
            Resource::plain_text("abcd↓efg", CTX.unwrap())
        }

        #[test]
        fn use_sender_if_given() {
            let mut mail = Mail::new_singlepart_mail(mock_resource());

            mail.insert_headers(
                headers! {
                    Sender: "strange@caffe.test",
                    _From: ["ape@caffe.test", "epa@caffe.test"],
                    _To: ["das@ding.test"]
                }
                .unwrap(),
            );

            let envelop_data = derive_envelop_data_from_mail(&mail).unwrap();

            assert_eq!(
                envelop_data.from.as_ref().unwrap().as_str(),
                "strange@caffe.test"
            );
        }

        #[test]
        fn use_from_if_no_sender_given() {
            let mut mail = Mail::new_singlepart_mail(mock_resource());
            mail.insert_headers(
                headers! {
                    _From: ["ape@caffe.test"],
                    _To: ["das@ding.test"]
                }
                .unwrap(),
            );

            let envelop_data = derive_envelop_data_from_mail(&mail).unwrap();

            assert_eq!(
                envelop_data.from.as_ref().unwrap().as_str(),
                "ape@caffe.test"
            );
        }

        #[test]
        fn fail_if_no_sender_but_multi_mailbox_from() {
            let mut mail = Mail::new_singlepart_mail(mock_resource());
            mail.insert_headers(
                headers! {
                    _From: ["ape@caffe.test", "a@b.test"],
                    _To: ["das@ding.test"]
                }
                .unwrap(),
            );

            let envelop_data = derive_envelop_data_from_mail(&mail);

            //assert is_err
            envelop_data.unwrap_err();
        }

        #[test]
        fn use_to() {
            let mut mail = Mail::new_singlepart_mail(mock_resource());
            mail.insert_headers(
                headers! {
                    _From: ["ape@caffe.test"],
                    _To: ["das@ding.test"]
                }
                .unwrap(),
            );

            let envelop_data = derive_envelop_data_from_mail(&mail).unwrap();

            assert_eq!(envelop_data.to.first().as_str(), "das@ding.test");
        }
    }

    mod mailaddress_from_mailbox {
        use super::super::mailaddress_from_mailbox;
        use headers::{
            header_components::{Email, Mailbox},
            HeaderTryFrom,
        };

        #[test]
        #[cfg_attr(not(feature = "test-with-traceing"), ignore)]
        fn does_not_panic_with_tracing_enabled() {
            let mb = Mailbox::try_from("hy@b").unwrap();
            mailaddress_from_mailbox(&mb).unwrap();
        }

        #[test]
        fn correctly_converts_mailbox() {
            let mb = Mailbox::from(Email::new("tast@tost.test").unwrap());
            let address = mailaddress_from_mailbox(&mb).unwrap();
            assert_eq!(address.as_str(), "tast@tost.test");
            assert_eq!(address.needs_smtputf8(), false);
        }

        #[test]
        fn tracks_if_smtputf8_is_needed() {
            let mb = Mailbox::from(Email::new("tüst@tost.test").unwrap());
            let address = mailaddress_from_mailbox(&mb).unwrap();
            assert_eq!(address.as_str(), "tüst@tost.test");
            assert_eq!(address.needs_smtputf8(), true);
        }

        #[test]
        fn puny_encodes_domain_if_smtputf8_is_not_needed() {
            let mb = Mailbox::from(Email::new("tast@tüst.test").unwrap());
            let address = mailaddress_from_mailbox(&mb).unwrap();
            assert_eq!(address.as_str(), "tast@xn--tst-hoa.test");
            assert_eq!(address.needs_smtputf8(), false);
        }

        #[test]
        fn does_not_puny_encodes_domain_if_smtputf8_is_needed() {
            let mb = Mailbox::from(Email::new("töst@tüst.test").unwrap());
            let address = mailaddress_from_mailbox(&mb).unwrap();
            assert_eq!(address.as_str(), "töst@tüst.test");
            assert_eq!(address.needs_smtputf8(), true);
        }
    }
}
