//! This library binds together `new-tokio-smtp` and the `mail` crates.
//!
//! It can be used to send mail given  as mail crates `Mail` instances
//! to a Mail Submission Agent (MSA). It could, theoretically also
//! be used to send to an MX, but this often needs additional functionality
//! for reliable usage which is not part of this crate.
//!
//! For ease of use this crate re-exports some of the most commonly used
//! parts from `new-tokio-smtp` including `ConnectionConfig`,
//! `ConnectionBuilder`, all authentication commands/methods (the
//! `auth` module) as well as useful types (in the `misc` module).
//!
//! The `send_mails` function is the simplest way to send a batch
//! of mails. Nevertheless it doesn't directly accept `Mail` instances,
//! instead it accepts `MailRequest` instances. This is needed, as
//! the sender/recipient(s) specified through the `Mail` headers
//! and those used fro smtp mail delivery are not necessary exactly
//! the same (e.g. for bounce back mails and some no-reply setups).
//!
//! # Example
//!
//! ```no_run
//! extern crate futures;
//! //if you use the mail facade use the re-exports from it instead
//! extern crate mail_core;
//! extern crate mail_smtp;
//! #[macro_use] extern crate mail_headers;
//!
//! use futures::Future;
//! use mail_headers::{
//!     headers::*,
//!     header_components::Domain
//! };
//! use mail_core::{Mail, default_impl::simple_context};
//! use mail_smtp::{self as smtp, ConnectionConfig};
//!
//! # fn main() {
//! // this is normally done _once per application instance_
//! // and then stored in e.g. a lazy_static. Also `Domain`
//! // will implement `FromStr` in the future.
//! let ctx = simple_context::new(Domain::from_unchecked("example.com".to_owned()), "asdkds".parse().unwrap())
//!     .unwrap();
//!
//! let mut mail = Mail::plain_text("Some body", &ctx);
//! mail.insert_headers(headers! {
//!     _From: ["bla@example.com"],
//!     _To: ["blub@example.com"],
//!     Subject: "Some Mail"
//! }.unwrap());
//!
//! // don't use unencrypted con for anything but testing and
//! // simplified examples
//! let con_config = ConnectionConfig::builder_local_unencrypted().build();
//!
//! let fut = smtp::send(mail.into(), con_config, ctx);
//! let results = fut.wait();
//! # }
//! ```
//!
//!
extern crate futures;
extern crate mail_core as mail;
#[cfg_attr(test, macro_use)]
extern crate mail_headers as headers;
extern crate mail_internals;
extern crate new_tokio_smtp;
#[macro_use]
extern crate failure;

mod resolve_all;

pub mod error;
mod request;
mod send_mail;

#[cfg(feature = "extended-api")]
pub use self::request::derive_envelop_data_from_mail;
pub use self::request::MailRequest;

#[cfg(feature = "extended-api")]
pub use self::send_mail::encode;
pub use self::send_mail::{send, send_batch};

pub use new_tokio_smtp::{ConnectionBuilder, ConnectionConfig};

pub mod auth {
    //! Module containing authentification commands/methods.
    //!
    //! This Module is re-exported from `new-tokio-smtp` for
    //! ease of use.

    pub use new_tokio_smtp::command::auth::*;

    /// Auth command for not doing anything on auth.
    //FIXME: this currently still sends the noop cmd,
    // replace it with some new "NoCommand" command.
    pub type NoAuth = ::new_tokio_smtp::command::Noop;
}

pub mod misc {
    //! A small collection of usefull types re-exported from `new-tokio-smtp`.
    pub use new_tokio_smtp::{AddressLiteral, ClientId, Cmd, DefaultTlsSetup, Domain, SetupTls};
}
