//! Provides the core mail type `Mail` for the `mail` crate.
//! This crate provides the type called `Mail` as well as ways
//! to create it. It also provides the builder context interface
//! and the `Resource` type, which is used to represent mail bodies.
//! Especially such which are attachments or embedded images.
//!
#![recursion_limit = "128"]

#[macro_use]
extern crate log;
#[macro_use]
extern crate failure;
extern crate checked_command;
extern crate chrono;
extern crate futures;
#[cfg_attr(test, macro_use)]
extern crate mail_headers as headers;
extern crate mail_internals as internals;
extern crate media_type;
extern crate rand;
extern crate soft_ascii_string;
extern crate vec1;

#[cfg(feature = "serde")]
extern crate serde;
#[cfg(all(test, feature = "serde"))]
extern crate serde_test;

#[cfg(feature = "default_impl_cpupool")]
extern crate futures_cpupool;
#[cfg(feature = "test-utils")]
extern crate lazy_static;

#[macro_use]
mod macros;
pub mod compose;
pub mod context;
mod encode;
pub mod error;
mod iri;
mod mail;
pub mod mime;
mod resource;
#[cfg(feature = "test-utils")]
pub mod test_utils;
pub mod utils;

pub mod default_impl;

pub use self::iri::IRI;
pub use self::mail::*;
pub use self::resource::*;

pub use context::{Context, MaybeEncData};

#[cfg(all(feature = "serde", not(feature = "serde-impl")))]
compile_error!(concat!(
    "\n---------------------------------------\n",
    " for serde use feature `serde-impl`,\n",
    " `serde` can not be used as feature in\n",
    " this crate due to limitations with Cargo\n",
    "-----------------------------------------\n"
));
