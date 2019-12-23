extern crate futures;
// Note that the `mail` crate provides a facade re-exporting
// all relevant parts.
extern crate mail_core;
extern crate mail_internals;
#[macro_use]
extern crate mail_headers;

use futures::Future;
use std::str;

use mail_internals::MailType;

// In the facade this is the `headers` module.
use mail_headers::{header_components::Domain, headers::*};

// In the facade this types (and the default_impl module)
// are also exposed at top level
use mail_core::{default_impl::simple_context, error::MailError, Mail};

fn print_some_mail() -> Result<(), MailError> {
    // Domain will implement `from_str` in the future,
    // currently it doesn't have a validator/parser.
    // So this will become `"example.com".parse()`
    let domain = Domain::from_unchecked("example.com".to_owned());
    // Normally you create this _once per application_.
    let ctx = simple_context::new(domain, "xqi93".parse().expect("we know it's ascii"))
        .expect("this is basically: failed to get cwd from env");

    let mut mail = Mail::plain_text("Hy there! 😁", &ctx);
    mail.insert_headers(headers! {
        _From: [("I'm Awesome 😁", "bla@examle.com")],
        _To: ["unknow@example.com"],
        Subject: "Hy there message 😁"
    }?);

    // We don't added any think which needs loading but we could have
    // and all of it would have been loaded concurrent and async.
    let encoded = mail
        .into_encodable_mail(ctx.clone())
        .wait()?
        .encode_into_bytes(MailType::Ascii)?;

    let mail_str = str::from_utf8(&encoded).unwrap();
    println!("{}", mail_str);
    Ok(())
}

fn main() {
    print_some_mail().unwrap()
}
