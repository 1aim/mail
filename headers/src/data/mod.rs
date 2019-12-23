//! A number of little helper types, which contain text.
//!
//! They provide mainly following functionality:
//!
//! 1. remember if the data is Ascii/Utf8
//!    - this might be extended at some point
//!      to contain non ascii data
//! 2. make sure the types are cheap to clone, by
//!    sharing the text internally.
//!    - this is mainly helpful when parsing a mail
//!
//! Both main points are for features which I decided to
//! to not yet implement, as such **there is a chance
//! that this module will be removed int the future**.
//!
mod inner_item;
pub use self::inner_item::*;

mod input;
pub use self::input::*;

mod simple_item;
pub use self::simple_item::*;
