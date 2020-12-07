use nom::IResult;
use std::fmt::{self, Display};

#[cfg(feature = "serde")]
use serde::{de::Error, Deserialize, Deserializer, Serialize, Serializer};
use soft_ascii_string::{SoftAsciiChar, SoftAsciiStr, SoftAsciiString};
use vec1::Vec1;

use data::{Input, SimpleItem};
use error::ComponentCreationError;
use internals::encoder::{EncodableInHeader, EncodingWriter};
use internals::error::EncodingError;
use {HeaderTryFrom, HeaderTryInto};

/// # Implementation Details
///
/// This is used for both message-id/content-id, but
/// depending on usage and support for obsolete parts there
/// are two "kind" of id's one which allows  FWS(/CFWS) in
/// some places and one which doesn't. This implementation
/// only supports the later one.
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct MessageId {
    message_id: SimpleItem,
}

impl MessageId {
    /// creates a message id from a string without checking for validity
    ///
    /// The string is expected to have the format `<left_part> "@" <right_part>`,
    /// i.e. it should not include the `"<"`, `">"` surrounding message id's in
    /// more or less all places they are used.
    pub fn from_unchecked(string: String) -> Self {
        let item = match SoftAsciiString::from_string(string) {
            Ok(ascii) => ascii.into(),
            Err(err) => err.into_source().into(),
        };

        MessageId { message_id: item }
    }

    pub fn new(
        left_part: &SoftAsciiStr,
        right_part: &SoftAsciiStr,
    ) -> Result<Self, ComponentCreationError> {
        use self::parser_parts as parser;

        match parser::id_left(left_part.as_str()) {
            IResult::Done("", _part) => {}
            _other => {
                return Err(ComponentCreationError::new_with_str(
                    "MessageId",
                    format!("{}@{}", left_part, right_part),
                ));
            }
        }

        match parser::id_right(right_part.as_str()) {
            IResult::Done("", _part) => {}
            _other => {
                return Err(ComponentCreationError::new_with_str(
                    "MessageId",
                    format!("{}@{}", left_part, right_part),
                ));
            }
        }

        let id = SoftAsciiString::from_unchecked(format!("{}@{}", left_part, right_part));
        let item = SimpleItem::Ascii(id.into());
        Ok(MessageId { message_id: item })
    }

    //FIXME make into AsRef<str> for MessageId
    pub fn as_str(&self) -> &str {
        self.message_id.as_str()
    }
}

#[cfg(feature = "serde")]
impl Serialize for MessageId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

#[cfg(feature = "serde")]
impl<'de> Deserialize<'de> for MessageId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let as_string = String::deserialize(deserializer)?;
        let as_ascii = SoftAsciiStr::from_str(&as_string)
            .map_err(|err| D::Error::custom(format!("message id is not ascii: {}", err)))?;

        let split_point = if as_ascii.as_str().ends_with("]") {
            as_ascii
                .as_str()
                .bytes()
                .rposition(|bch| bch == b'[')
                .and_then(|pos| pos.checked_sub(1))
                .ok_or_else(|| D::Error::custom("invalid message id format"))?
        } else {
            as_ascii
                .as_str()
                .bytes()
                .rposition(|bch| bch == b'@')
                .ok_or_else(|| D::Error::custom("invalid message id format"))?
        };

        let left_part = &as_ascii[..split_point];
        let right_part = &as_ascii[split_point + 1..];
        MessageId::new(left_part, right_part)
            .map_err(|err| D::Error::custom(format!("invalid message id format: {}", err)))
    }
}

impl Display for MessageId {
    fn fmt(&self, fter: &mut fmt::Formatter) -> fmt::Result {
        fter.write_str(self.as_str())
    }
}

impl<T> HeaderTryFrom<T> for MessageId
where
    T: HeaderTryInto<Input>,
{
    fn try_from(input: T) -> Result<Self, ComponentCreationError> {
        use self::parser_parts::parse_message_id;

        let input = input.try_into()?;

        match parse_message_id(input.as_str()) {
            IResult::Done("", _msg_id) => {}
            _other => {
                return Err(ComponentCreationError::new_with_str(
                    "MessageId",
                    input.as_str(),
                ));
            }
        }

        Ok(MessageId {
            message_id: input.into(),
        })
    }
}

impl EncodableInHeader for MessageId {
    fn encode(&self, handle: &mut EncodingWriter) -> Result<(), EncodingError> {
        handle.mark_fws_pos();
        handle.write_char(SoftAsciiChar::from_unchecked('<'))?;
        match self.message_id {
            SimpleItem::Ascii(ref ascii) => handle.write_str(ascii)?,
            SimpleItem::Utf8(ref utf8) => handle.write_utf8(utf8)?,
        }
        handle.write_char(SoftAsciiChar::from_unchecked('>'))?;
        handle.mark_fws_pos();
        Ok(())
    }

    fn boxed_clone(&self) -> Box<EncodableInHeader> {
        Box::new(self.clone())
    }
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct MessageIdList(pub Vec1<MessageId>);

deref0! { +mut MessageIdList => Vec1<MessageId> }

impl EncodableInHeader for MessageIdList {
    fn encode(&self, handle: &mut EncodingWriter) -> Result<(), EncodingError> {
        for msg_id in self.iter() {
            msg_id.encode(handle)?;
        }
        Ok(())
    }

    fn boxed_clone(&self) -> Box<EncodableInHeader> {
        Box::new(self.clone())
    }
}

mod parser_parts {
    use internals::grammar::{is_atext, is_dtext};
    use internals::MailType;
    use nom::IResult;

    pub fn parse_message_id(input: &str) -> IResult<&str, (&str, &str)> {
        do_parse!(input, l: id_left >> char!('@') >> r: id_right >> (l, r))
    }

    pub fn id_left(input: &str) -> IResult<&str, &str> {
        dot_atom_text(input)
    }

    pub fn id_right(input: &str) -> IResult<&str, &str> {
        alt!(input, no_fold_literal | dot_atom_text)
    }

    fn no_fold_literal(input: &str) -> IResult<&str, &str> {
        recognize!(
            input,
            tuple!(
                char!('['),
                take_while!(call!(is_dtext, MailType::Internationalized)),
                char!(']')
            )
        )
    }

    fn dot_atom_text(input: &str) -> IResult<&str, &str> {
        recognize!(
            input,
            tuple!(
                take_while1!(call!(is_atext, MailType::Internationalized)),
                many0!(tuple!(
                    char!('.'),
                    take_while1!(call!(is_atext, MailType::Internationalized))
                ))
            )
        )
    }

    #[cfg(test)]
    mod test {
        use super::*;
        use nom;

        #[test]
        fn rec_dot_atom_text_no_dot() {
            match dot_atom_text("abc") {
                IResult::Done("", "abc") => {}
                other => panic!("excepted Done(\"\",\"abc\") got {:?}", other),
            }
        }

        #[test]
        fn rec_dot_atom_text_dots() {
            match dot_atom_text("abc.def.ghi") {
                IResult::Done("", "abc.def.ghi") => {}
                other => panic!("excepted Done(\"\",\"abc.def.ghi\") got {:?}", other),
            }
        }

        #[test]
        fn rec_dot_atom_text_no_end_dot() {
            let test_str = "abc.";
            let need_size = test_str.len() + 1;
            match dot_atom_text(test_str) {
                IResult::Incomplete(nom::Needed::Size(ns)) if ns == need_size => {}
                other => panic!("excepted Incomplete(Complete) got {:?}", other),
            }
        }

        #[test]
        fn rec_dot_atom_text_no_douple_dot() {
            match dot_atom_text("abc..de") {
                IResult::Done("..de", "abc") => {}
                other => panic!("excepted Done(\"..de\",\"abc\") got {:?}", other),
            }
        }

        #[test]
        fn rec_dot_atom_text_no_start_dot() {
            match dot_atom_text(".abc") {
                IResult::Error(..) => {}
                other => panic!("expected error got {:?}", other),
            }
        }

        #[test]
        fn no_empty() {
            match dot_atom_text("") {
                IResult::Incomplete(nom::Needed::Size(1)) => {}
                other => panic!("excepted Incomplete(Size(1)) got {:?}", other),
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use internals::encoder::EncodingBuffer;
    use internals::MailType;

    ec_test! { new, {
        MessageId::new(
            SoftAsciiStr::from_unchecked("just.me"),
            SoftAsciiStr::from_unchecked("[127.0.0.1]")
        )?
    } => ascii => [
        MarkFWS,
        Text "<just.me@[127.0.0.1]>",
        MarkFWS
    ]}

    ec_test! { simple, {
        MessageId::try_from( "affen@haus" )?
    } => ascii => [
        MarkFWS,
        // there are two "context" one which allows FWS inside (defined = email)
        // and one which doesn't for simplicity we use the later every where
        Text "<affen@haus>",
        MarkFWS
    ]}

    ec_test! { utf8, {
        MessageId::try_from( "↓@↑.utf8")?
    } => utf8 => [
        MarkFWS,
        Text "<↓@↑.utf8>",
        MarkFWS
    ]}

    #[test]
    fn utf8_fails() {
        let mut encoder = EncodingBuffer::new(MailType::Ascii);
        let mut handle = encoder.writer();
        let mid = MessageId::try_from("abc@øpunny.code").unwrap();
        assert_err!(mid.encode(&mut handle));
        handle.undo_header();
    }

    ec_test! { multipls, {
        let fst = MessageId::try_from( "affen@haus" )?;
        let snd = MessageId::try_from( "obst@salat" )?;
        MessageIdList( vec1! [
            fst,
            snd
        ])
    } => ascii => [
        MarkFWS,
        Text "<affen@haus>",
        MarkFWS,
        MarkFWS,
        Text "<obst@salat>",
        MarkFWS,
    ]}
}
