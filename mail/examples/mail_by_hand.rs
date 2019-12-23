//! In this example a `Mail` is directly created encoded and
//! printed

#[macro_use]
extern crate mail;
extern crate futures;
extern crate soft_ascii_string;

use futures::Future;
use soft_ascii_string::SoftAsciiString;

use std::str;

use mail::{
    default_impl::simple_context, error::MailError, Context, Domain, HeaderTryFrom, Mail, MailType,
    Resource,
};

// Mail uses \r\n newlines!!
const MSG: &str = "Dear Tree Apes,\r
\r
the next grate block buster is here 🎉\r
\r
With regards,\r
The Tree Movie Consortium\r
";

fn create_text_body(ctx: &impl Context) -> Resource {
    Resource::plain_text(MSG, ctx)
}

fn build_mail(ctx: &impl Context) -> Result<Mail, MailError> {
    use mail::headers::*;

    let mut mail = Mail::new_singlepart_mail(create_text_body(ctx));
    mail.insert_headers(headers! {
        // `From` can have more than one mailbox.
        _From: [("Tree Movie Consortium", "datmail@dat.test")],
        // `To` can have more then one mailbox.
        _To: [("Tree Ape Chief", "datothermail@dat.test")],
        Subject: "Rise of the Type Lord: The 🙈 Emoji Plight"
        // `Date`, `ContentType`, `ContentTransferEncoding` get added automatically.
    }?);

    Ok(mail)
}

fn encode_mail_to_stdout(mail: Mail, ctx: impl Context) -> Result<(), MailError> {
    let bytes = mail
        // This loads lazy resources, e.g. attachments/embeddings.
        .into_encodable_mail(ctx)
        // It's a future, but we will just block here.
        .wait()?
        // Encodes mail and returns a `Vec<u8>` representing the
        // mail. `Vec<u8>` is used as mails can contain non-utf8
        // data through a number of ways, though in many (most?)
        // situations today they should not. (Also with mail type
        // Ascii it can not contain non ascii chars without being
        // invalid but with 8BITMIME and Internationalized mails
        // it can).
        .encode_into_bytes(MailType::Ascii)?;

    // Note how the 🙈 utf8 char will be automatically encoded (it would not be
    // specially encoded if MailType would be Internationalized).
    println!(
        "{}",
        str::from_utf8(bytes.as_slice()).expect("[BUG] MailType::Ascii can't have non utf8 bytes")
    );
    Ok(())
}

fn main() {
    println!("---------------- START ---------------- ");
    let msg_id_domain = Domain::try_from("company_a.test").unwrap();
    let unique_part = SoftAsciiString::from_string("c207n521cec").unwrap();
    let ctx = simple_context::new(msg_id_domain, unique_part).unwrap();

    let mail = build_mail(&ctx).unwrap();
    encode_mail_to_stdout(mail, ctx).unwrap();
    println!("----------------  END  ---------------- ");
}
