//! In this examples runs a simple command line dialog to create a mail
//! and send it to an MSA

#[cfg(not(feature = "smtp"))]
compile_error!("example `send_mail` requires feature `smtp`");

#[macro_use]
extern crate mail;
extern crate futures;
extern crate rpassword;
extern crate soft_ascii_string;
extern crate tokio;

use std::io::{self, Write};

use futures::{future, Stream};
use soft_ascii_string::SoftAsciiString;

use mail::{
    default_impl::simple_context,
    error::MailError,
    header_components::Domain,
    smtp::{
        auth::Plain as AuthPlain, send_batch as send_mail_batch, ConnectionBuilder,
        ConnectionConfig, MailRequest,
    },
    Context, HeaderTryFrom, Mail,
};

mod cli;

fn main() {
    let msg_id_domain = Domain::try_from("company_a.test").unwrap();
    let unique_part = SoftAsciiString::from_string("c207n521cec").unwrap();
    let ctx = simple_context::new(msg_id_domain, unique_part).unwrap();
    let (msa_info, mails) = read_data().unwrap();
    let connection_config = create_connection_config(msa_info);
    let mail_requests = create_mail_requests(mails, &ctx).unwrap();

    println!("[starting sending mails]");

    // We run a tokio core "just" for sending the mails,
    // normally we probably would schedule/spawn this task
    // on a existing tokio runtime.
    tokio::run(future::lazy(move || {
        send_mail_batch(mail_requests, connection_config, ctx)
            .then(|res| Ok(res))
            .for_each(|res| {
                match res {
                    Ok(_) => println!("[mail send]"),
                    Err(err) => println!("[sending mail failed] {:?}", err),
                }
                Ok(())
            })
    }));

    println!("[DONE]");
}

fn create_connection_config(msa_info: cli::MsaInfo) -> ConnectionConfig<AuthPlain> {
    let cli::MsaInfo { domain, auth } = msa_info;

    ConnectionBuilder::new(domain)
        .expect("could not resolve domain/host name of MSA")
        .auth(
            AuthPlain::from_username(auth.username, auth.password)
                .expect("used \\0 in username or password"),
        )
        .build()
}

fn create_mail_requests(
    mails: Vec<cli::SimpleMail>,
    ctx: &impl Context,
) -> Result<Vec<MailRequest>, MailError> {
    use mail::headers::*;

    let requests = mails
        .into_iter()
        .map(|simple_mail| {
            let cli::SimpleMail {
                from,
                to,
                subject,
                text_body,
            } = simple_mail;
            let mut mail = Mail::plain_text(text_body, ctx);
            mail.insert_headers(headers! {
                _From: [from],
                _To: [to],
                Subject: subject
            }?);

            Ok(MailRequest::from(mail))
        })
        .collect::<Result<Vec<_>, _>>();

    requests
}

fn read_data() -> Result<(cli::MsaInfo, Vec<cli::SimpleMail>), io::Error> {
    cli::with_dialog(|mut dialog| {
        let msa_info = dialog.read_msa_info()?;
        let mut mails = Vec::new();
        let mut more = true;

        writeln!(dialog.stdout(), "\nreading mails:")?;
        while more {
            let mail = dialog.read_simple_mail()?;
            mails.push(mail);
            dialog.prompt("Another Mail? [y/n]")?;
            more = dialog.read_yn()?;
        }
        writeln!(dialog.stdout(), "[collecting data done]")?;
        Ok((msa_info, mails))
    })
}
