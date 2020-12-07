use std::borrow::Cow;
#[cfg(feature = "serde")]
use std::fmt;

use failure::Fail;
use media_type::push_params_to_buffer;
use media_type::spec::{Ascii, Internationalized, MimeSpec, Modern};
use soft_ascii_string::SoftAsciiStr;

#[cfg(feature = "serde")]
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use error::ComponentCreationError;
use internals::encoder::{EncodableInHeader, EncodingWriter};
use internals::error::{EncodingError, EncodingErrorKind};
use HeaderTryFrom;

use super::FileMeta;

/// Disposition Component mainly used for the Content-Disposition header (rfc2183)
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Disposition {
    kind: DispositionKind,
    file_meta: DispositionParameters,
}

#[derive(Debug, Clone, Eq, PartialEq, Hash, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
struct DispositionParameters(FileMeta);

/// Represents what kind of disposition is used (Inline/Attachment)
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum DispositionKind {
    /// Display the body "inline".
    ///
    /// This disposition is mainly used to add some additional content
    /// and then refers to it through its cid (e.g. in a html mail).
    Inline,
    /// Display the body as an attachment to of the mail.
    Attachment,
}

impl Disposition {
    /// Create a inline disposition with default parameters.
    pub fn inline() -> Self {
        Disposition::new(DispositionKind::Inline, FileMeta::default())
    }

    /// Create a attachment disposition with default parameters.
    pub fn attachment() -> Self {
        Disposition::new(DispositionKind::Attachment, FileMeta::default())
    }

    /// Create a new disposition with given parameters.
    pub fn new(kind: DispositionKind, file_meta: FileMeta) -> Self {
        Disposition {
            kind,
            file_meta: DispositionParameters(file_meta),
        }
    }

    /// Return which kind of disposition this represents.
    pub fn kind(&self) -> DispositionKind {
        self.kind
    }

    /// Returns the parameters associated with the disposition.
    pub fn file_meta(&self) -> &FileMeta {
        &self.file_meta
    }

    /// Returns a mutable reference to the parameters associated with the disposition.
    pub fn file_meta_mut(&mut self) -> &mut FileMeta {
        &mut self.file_meta
    }
}

#[cfg(feature = "serde")]
impl Serialize for DispositionKind {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            &DispositionKind::Inline => serializer.serialize_str("inline"),
            &DispositionKind::Attachment => serializer.serialize_str("attachment"),
        }
    }
}

#[cfg(feature = "serde")]
impl<'de> Deserialize<'de> for DispositionKind {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct Visitor;
        impl<'de> ::serde::de::Visitor<'de> for Visitor {
            type Value = DispositionKind;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("\"inline\" or \"attachment\"")
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: ::serde::de::Error,
            {
                if value.eq_ignore_ascii_case("inline") {
                    Ok(DispositionKind::Inline)
                } else if value.eq_ignore_ascii_case("attachment") {
                    Ok(DispositionKind::Attachment)
                } else {
                    Err(E::custom(format!("unknown disposition: {:?}", value)))
                }
            }
        }

        deserializer.deserialize_str(Visitor)
    }
}

/// This try from is for usability only, it is
/// generally recommendet to use Disposition::inline()/::attachment()
/// as it is type safe / compiler time checked, while this one
/// isn't
impl<'a> HeaderTryFrom<&'a str> for Disposition {
    fn try_from(text: &'a str) -> Result<Self, ComponentCreationError> {
        if text.eq_ignore_ascii_case("Inline") {
            Ok(Disposition::inline())
        } else if text.eq_ignore_ascii_case("Attachment") {
            Ok(Disposition::attachment())
        } else {
            let mut err = ComponentCreationError::new("Disposition");
            err.set_str_context(text);
            return Err(err);
        }
    }
}

//TODO provide a gnneral way for encoding header parameter ...
//  which follow the scheme: <mainvalue> *(";" <key>"="<value> )
//  this are: ContentType and ContentDisposition for now
impl EncodableInHeader for DispositionParameters {
    fn encode(&self, handle: &mut EncodingWriter) -> Result<(), EncodingError> {
        let mut params = Vec::<(&str, Cow<str>)>::new();
        if let Some(filename) = self.file_name.as_ref() {
            params.push(("filename", Cow::Borrowed(filename)));
        }
        if let Some(creation_date) = self.creation_date.as_ref() {
            params.push(("creation-date", Cow::Owned(creation_date.to_rfc2822())));
        }
        if let Some(date) = self.modification_date.as_ref() {
            params.push(("modification-date", Cow::Owned(date.to_rfc2822())));
        }
        if let Some(date) = self.read_date.as_ref() {
            params.push(("read-date", Cow::Owned(date.to_rfc2822())));
        }
        if let Some(size) = self.size.as_ref() {
            params.push(("size", Cow::Owned(size.to_string())));
        }

        //TODO instead do optCFWS ; spCFWS <name>=<value>
        // so that soft line brakes can be done
        let mut buff = String::new();
        let res = if handle.mail_type().is_internationalized() {
            push_params_to_buffer::<MimeSpec<Internationalized, Modern>, _, _, _>(&mut buff, params)
        } else {
            push_params_to_buffer::<MimeSpec<Ascii, Modern>, _, _, _>(&mut buff, params)
        };

        match res {
            Err(err) => Err(err.context(EncodingErrorKind::Malformed).into()),
            Ok(_) => {
                handle.write_str_unchecked(&*buff)?;
                Ok(())
            }
        }
    }

    fn boxed_clone(&self) -> Box<EncodableInHeader> {
        Box::new(self.clone())
    }
}

impl EncodableInHeader for Disposition {
    fn encode(&self, handle: &mut EncodingWriter) -> Result<(), EncodingError> {
        use self::DispositionKind::*;
        match self.kind {
            Inline => {
                handle.write_str(SoftAsciiStr::from_unchecked("inline"))?;
            }
            Attachment => {
                handle.write_str(SoftAsciiStr::from_unchecked("attachment"))?;
            }
        }
        self.file_meta.encode(handle)?;
        Ok(())
    }

    fn boxed_clone(&self) -> Box<EncodableInHeader> {
        Box::new(self.clone())
    }
}

deref0! {+mut DispositionParameters => FileMeta }

#[cfg(test)]
mod test {
    use chrono;
    use std::default::Default;

    use super::*;

    pub fn test_time(modif: u32) -> chrono::DateTime<chrono::Utc> {
        use chrono::prelude::*;
        Utc.ymd(2013, 8, 6).and_hms(7, 11, modif)
    }

    ec_test! { no_params_inline, {
        Disposition::inline()
    } => ascii => [
        Text "inline"
    ]}

    ec_test! { no_params_attachment, {
        Disposition::attachment()
    } => ascii => [
        Text "attachment"
    ]}

    ec_test! { attachment_encode_file_name, {
        Disposition::new( DispositionKind::Attachment, FileMeta {
            file_name: Some("this is nice".to_owned()),
            ..Default::default()
        })
    } => ascii => [
        Text "attachment; filename=\"this is nice\""
    ]}

    ec_test! { attachment_all_params, {
        Disposition::new( DispositionKind::Attachment, FileMeta {
            file_name: Some( "random.png".to_owned() ),
            creation_date: Some( test_time( 1 ) ),
            modification_date: Some( test_time( 2 ) ),
            read_date: Some( test_time( 3 ) ),
            size: Some( 4096 )
        })
    } => ascii => [
        Text concat!( "attachment",
            "; filename=random.png",
            "; creation-date=\"Tue,  6 Aug 2013 07:11:01 +0000\"",
            "; modification-date=\"Tue,  6 Aug 2013 07:11:02 +0000\"",
            "; read-date=\"Tue,  6 Aug 2013 07:11:03 +0000\"",
            "; size=4096" ),
    ]}

    ec_test! { inline_file_name_param, {
        Disposition::new(DispositionKind::Inline, FileMeta {
            file_name: Some("logo.png".to_owned()),
            ..Default::default()
        })
    } => ascii => [
        Text "inline; filename=logo.png"
    ]}
    //TODO: (1 allow FWS or so in parameters) (2 utf8 file names)

    #[test]
    fn test_from_str() {
        assert_ok!(Disposition::try_from("Inline"));
        assert_ok!(Disposition::try_from("InLine"));
        assert_ok!(Disposition::try_from("Attachment"));

        assert_err!(Disposition::try_from("In line"));
    }

    #[cfg(feature = "serde")]
    fn assert_serialize<S: ::serde::Serialize>() {}
    #[cfg(feature = "serde")]
    fn assert_deserialize<S: ::serde::Serialize>() {}

    #[cfg(feature = "serde")]
    #[test]
    fn disposition_serialization() {
        assert_serialize::<Disposition>();
        assert_serialize::<DispositionKind>();
        assert_serialize::<DispositionParameters>();
        assert_deserialize::<Disposition>();
        assert_deserialize::<DispositionKind>();
        assert_deserialize::<DispositionParameters>();
    }
}
