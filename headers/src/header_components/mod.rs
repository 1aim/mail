//! This modules contains all components provided by this library.
//!
//! A mail (header) component is basically the body of a header field
//! in the mails header section. E.g. in `Subject: Hy There`, the
//! `Hy There` would be represented by an component (in this case
//! the `Unstructured` component).
//!
//!
pub mod utils;

mod file_meta;
pub use self::file_meta::*;

//reexport our components
mod date_time;
pub use self::date_time::DateTime;

mod email;
pub use self::email::{Domain, Email, LocalPart};

mod mailbox;
pub use self::mailbox::{Mailbox, NoDisplayName};

mod mailbox_list;
pub use self::mailbox_list::{MailboxList, OptMailboxList};

mod transfer_encoding;
pub use self::transfer_encoding::TransferEncoding;

mod unstructured;
pub use self::unstructured::Unstructured;

mod message_id;
pub use self::message_id::{MessageId, MessageIdList};

pub type ContentId = MessageId;
pub type ContentIdList = MessageIdList;

mod cfws;
pub use self::cfws::{CFWS, FWS};

mod media_type;
pub use self::media_type::*;

pub type ContentType = MediaType;

mod path;
pub use self::path::Path;

mod received_token;
pub use self::received_token::ReceivedToken;

pub mod word;
pub use self::word::Word;

mod phrase;
pub use self::phrase::Phrase;

mod phrase_list;
pub use self::phrase_list::PhraseList;

mod disposition;
pub use self::disposition::*;

mod raw_unstructured;
pub use self::raw_unstructured::*;
