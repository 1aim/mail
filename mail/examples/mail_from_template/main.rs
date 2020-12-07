//! In this example a mail is created using the template engine with handlebars bindings
//! (and then printed)
//!

extern crate failure;
extern crate futures;
extern crate mail;
extern crate serde;
extern crate soft_ascii_string;

use std::path::Path;

use failure::Error;
use futures::Future;
use soft_ascii_string::SoftAsciiString;

use serde::Serialize;

use mail::{
    default_impl::simple_context,
    headers,
    template::{handlebars::Handlebars, load_toml_template_from_path, TemplateExt},
    Domain, HeaderKind, HeaderTryFrom, MailType,
};

#[derive(Debug, Serialize)]
struct UserData {
    name: &'static str,
}

fn main() {
    //TODO[workspace] check if [dir mail and has sub-dir mail]/[dir mail and has parent mail]
    // depending on this switch the CWD to the subdir-mail
    eprintln!(concat!(
        "Warning: This example currently only works if mail/mail is the cwd\n",
        "  i.e. it fails when run from the workspace!"
    ));
    let msg_id_domain = Domain::try_from("company_a.test").unwrap();
    let unique_part = SoftAsciiString::from_string("c207n521cec").unwrap();
    let ctx = simple_context::new(msg_id_domain, unique_part).unwrap();

    let engine = Handlebars::new();
    let template_path =
        Path::new("example_resources/templates/template_a/template.toml").to_owned();
    let fut = load_toml_template_from_path(engine, template_path, &ctx)
        .and_then(|template| -> Result<_, Error> {
            // If our "user data" would contain additional inline_emebeddings/attachments
            // we would need another "and_then" chain here where we load such parts.
            let mut mail = template.render(UserData { name: "Ferris" }.into(), &ctx)?;
            // `auto_body` has some nice but failable conversion for **header bodies**.
            // Alternatively there is `<HeaderKind>::body(<SomeHeaderBodyType>)` which
            // is not failable.
            mail.insert_header(headers::_From::auto_body(["a@b.example"])?);
            mail.insert_header(headers::_To::auto_body(["d@e.example"])?);
            Ok(mail)
        })
        .and_then(|mail| mail.into_encodable_mail(ctx.clone()).map_err(Into::into));

    let enc_mail = fut.wait().unwrap();
    let bytes = enc_mail.encode_into_bytes(MailType::Ascii).unwrap();
    let string = String::from_utf8(bytes).unwrap();
    println!("{}", string);
}
