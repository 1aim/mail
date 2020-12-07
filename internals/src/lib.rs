//! Provides some internal functionality for the `mail` crate.
#![recursion_limit = "256"]
extern crate failure;
#[macro_use]
extern crate nom;
extern crate base64;
extern crate chrono;
extern crate idna;
extern crate media_type;
extern crate media_type_impl_utils;
extern crate percent_encoding;
extern crate quoted_printable;
extern crate quoted_string;
extern crate soft_ascii_string;
extern crate vec1;

//NOTE: this would be worth it's own independent crate for utility macros
#[macro_use]
mod macros;
#[macro_use]
pub mod utils;
mod mail_type;
#[macro_use]
pub mod error;
pub mod grammar;
//NOTE: encoder is in the order _above_ bind, i.e. bind can import the encoder,
//  but the encoder should not import anything from bind!
pub mod bind;
#[cfg_attr(test, macro_use)]
pub mod encoder;

pub use self::mail_type::*;

#[cfg(all(test, not(feature = "traceing")))]
compile_error! { "testing needs feature `traceing` to be enabled" }

//reexports for exported macros
#[doc(hidden)]
pub use failure::Error as __FError;
