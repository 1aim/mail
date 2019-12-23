#[cfg(feature = "serde")]
use std::fmt;
use std::{ops::Deref, str::FromStr};

#[cfg(feature = "serde")]
use serde::{
    de::{self, Visitor},
    Deserialize, Deserializer, Serialize, Serializer,
};
use soft_ascii_string::{SoftAsciiChar, SoftAsciiStr};

use internals::{
    encoder::{EncodableInHeader, EncodingWriter},
    error::EncodingError,
};
use media_type::{
    spec::{Ascii, Internationalized, MimeSpec, Modern},
    AnyMediaType, MediaType as _MediaType, Name,
};

use crate::{error::ComponentCreationError, HeaderTryFrom};

#[derive(Debug, Clone)]
pub struct MediaType {
    media_type: InternationalizedMediaType,
    might_need_utf8: bool,
}

impl MediaType {
    pub fn parse(media_type: &str) -> Result<Self, ComponentCreationError> {
        let media_type = InternationalizedMediaType::parse(media_type).map_err(|e| {
            ComponentCreationError::from_parent(e.to_owned(), "MediaType")
                .with_str_context(media_type)
        })?;

        Ok(media_type.into())
    }

    pub fn new<T, ST>(type_: T, subtype: ST) -> Result<Self, ComponentCreationError>
    where
        T: AsRef<str>,
        ST: AsRef<str>,
    {
        let media_type = AsciiMediaType::new(type_.as_ref(), subtype.as_ref()).map_err(|e| {
            ComponentCreationError::from_parent(e, "MediaType").with_str_context(format!(
                "{}/{}",
                type_.as_ref(),
                subtype.as_ref()
            ))
        })?;

        Ok(media_type.into())
    }

    pub fn new_with_params<T, ST, I, IV, IN>(
        type_: T,
        subtype: ST,
        params: I,
    ) -> Result<Self, ComponentCreationError>
    where
        T: AsRef<str>,
        ST: AsRef<str>,
        I: IntoIterator<Item = (IV, IN)>,
        IV: AsRef<str>,
        IN: AsRef<str>,
    {
        let media_type =
            InternationalizedMediaType::new_with_params(type_.as_ref(), subtype.as_ref(), params)
                .map_err(|e| {
                ComponentCreationError::from_parent(e, "MediaType").with_str_context(format!(
                    "{}/{} <params-eluded>",
                    type_.as_ref(),
                    subtype.as_ref()
                ))
            })?;

        Ok(media_type.into())
    }

    pub fn remove_param<N>(&mut self, name: N) -> bool
    where
        N: for<'a> PartialEq<Name<'a>>,
    {
        self.media_type.remove_param(name)
    }

    pub fn set_param<N, V>(&mut self, name: N, value: V)
    where
        N: AsRef<str>,
        V: AsRef<str>,
    {
        self.media_type.set_param(name, value)
    }
}

impl FromStr for MediaType {
    type Err = ComponentCreationError;
    fn from_str(inp: &str) -> Result<Self, Self::Err> {
        MediaType::parse(inp)
    }
}

impl Deref for MediaType {
    type Target = AnyMediaType;

    fn deref(&self) -> &Self::Target {
        &self.media_type
    }
}

type AsciiMediaType = _MediaType<MimeSpec<Ascii, Modern>>;
type InternationalizedMediaType = _MediaType<MimeSpec<Internationalized, Modern>>;

impl From<AsciiMediaType> for MediaType {
    fn from(media_type: AsciiMediaType) -> Self {
        MediaType {
            media_type: media_type.into(),
            might_need_utf8: false,
        }
    }
}

impl From<InternationalizedMediaType> for MediaType {
    fn from(media_type: InternationalizedMediaType) -> Self {
        MediaType {
            media_type: media_type,
            might_need_utf8: true,
        }
    }
}

impl Into<AnyMediaType> for MediaType {
    fn into(self) -> AnyMediaType {
        self.media_type.into()
    }
}

impl Into<InternationalizedMediaType> for MediaType {
    fn into(self) -> InternationalizedMediaType {
        self.media_type
    }
}

impl HeaderTryFrom<AsciiMediaType> for MediaType {
    fn try_from(mime: AsciiMediaType) -> Result<Self, ComponentCreationError> {
        Ok(mime.into())
    }
}

impl HeaderTryFrom<InternationalizedMediaType> for MediaType {
    fn try_from(mime: InternationalizedMediaType) -> Result<Self, ComponentCreationError> {
        Ok(mime.into())
    }
}

impl<'a> HeaderTryFrom<&'a str> for MediaType {
    fn try_from(media_type: &'a str) -> Result<Self, ComponentCreationError> {
        Self::parse(media_type)
    }
}

impl EncodableInHeader for MediaType {
    fn encode(&self, handle: &mut EncodingWriter) -> Result<(), EncodingError> {
        let no_recheck_needed = handle.mail_type().is_internationalized() || !self.might_need_utf8;

        //type and subtype are always ascii
        handle.write_str(SoftAsciiStr::from_unchecked(self.type_().as_ref()))?;
        handle.write_char(SoftAsciiChar::from_unchecked('/'))?;
        handle.write_str(SoftAsciiStr::from_unchecked(self.subtype().as_ref()))?;
        for (name, value) in self.params() {
            //FIXME for now do not split params at all
            handle.mark_fws_pos();
            handle.write_char(SoftAsciiChar::from_unchecked(';'))?;
            handle.write_fws();
            //names are always ascii
            handle.write_str(SoftAsciiStr::from_unchecked(name.as_ref()))?;

            handle.write_char(SoftAsciiChar::from_unchecked('='))?;
            if no_recheck_needed {
                handle.write_str_unchecked(value.as_str_repr())?;
            } else {
                match SoftAsciiStr::from_str(value.as_str_repr()) {
                    Ok(soa) => handle.write_str(soa)?,
                    Err(_) => {
                        //TODO encode value ! then write it
                        unimplemented!();
                    }
                }
            }
        }
        Ok(())
    }

    fn boxed_clone(&self) -> Box<EncodableInHeader> {
        Box::new(self.clone())
    }
}

#[cfg(feature = "serde")]
impl Serialize for MediaType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str_repr())
    }
}

#[cfg(feature = "serde")]
impl<'de> Deserialize<'de> for MediaType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct MediaTypeVisitor;

        impl<'de> Visitor<'de> for MediaTypeVisitor {
            type Value = MediaType;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                write!(formatter, "a string representing a MediaType")
            }

            fn visit_str<E>(self, s: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                let mt = s.parse().map_err(|err| E::custom(err))?;

                Ok(mt)
            }
        }

        deserializer.deserialize_str(MediaTypeVisitor)
    }
}

//
///// encodes all non ascii parts of a mime turning it into an ascii mime
/////
//fn encode_mime(mime: &MediaType, handle: &mut EncodingWriter) -> Result<()> {
//    //TODO(upstream=mime): this can be simplified with upstem fixes to the mime crate
//    handle.write_str(SoftAsciiStr::from_unchecked(mime.type_().as_str()))?;
//    handle.write_char(SoftAsciiChar::from_unchecked('/'))?;
//    handle.write_str(SoftAsciiStr::from_unchecked(mime.subtype().as_str()))?;
//
//    let mail_type = handle.mail_type();
//    let mut split_params = HashMap::new();
//
//    for (name, value) in mime.params() {
//        let (split_num, is_encoded) = get_split_num(&name)?;
//        if let Some((name, section)) = split_num {
//            // as the charset can only be set in the first of multiple splits and
//            // the first does not have to be the first in the iteration we have to
//            // delay the handling
//            let old = split_params
//                .entry(name)
//                .or_insert(HashMap::new())
//                .insert(section, (value,  is_encoded));
//
//            if old.is_some() {
//                bail!(InvalidMime(mime.to_string()))
//            }
//        } else {
//            handle.mark_fws_pos();
//            handle.write_char(SoftAsciiChar::from_unchecked(';'))?;
//            handle.mark_fws_pos();
//            if is_encoded {
//                //parameter names are ascii, values might not be ascii
//                handle.write_str(SoftAsciiStr::from_unchecked(name.as_str()))?;
//                handle.write_char(SoftAsciiChar::from_unchecked('='))?;
//                if let Ok(asciied) = SoftAsciiStr::from_str(value.as_str()) {
//                    handle.write_str(asciied)?;
//                } else {
//                    bail!(InvalidMime(mime.to_string()))
//                }
//            } else {
//                // this whole reparsing can be storngly simplified if the mime crate would
//                // returns either the content OR the underlying representation for as_str,
//                // but it returns something in between...
//                let mut token = true;
//                let mut qtext = true;
//                let mut had_slash = false;
//                for ch in value.as_str().chars() {
//                    if token { token = is_token_char(ch) }
//                    if qtext {
//                        if had_slash {
//                            qtext = is_vchar(ch, mail_type) || is_ws(ch);
//                            had_slash = false;
//                        } else if ch == '\\' {
//                            had_slash = true;
//                        } else {
//                            qtext = is_qtext(ch, mail_type) || is_ws(ch)
//                        }
//                    }
//                }
//                qtext = qtext & !had_slash;
//
//                if token || qtext {
//                    handle.write_str(SoftAsciiStr::from_unchecked(name.as_str()))?;
//                    handle.write_char(SoftAsciiChar::from_unchecked('='))?;
//                    if token {
//                        handle.write_str(SoftAsciiStr::from_unchecked(value.as_str()))?;
//                    } else if qtext {
//                        handle.write_char(SoftAsciiChar::from_unchecked('\"'))?;
//                        handle.write_str_unchecked(value.as_str())?;
//                        handle.write_char(SoftAsciiChar::from_unchecked('\"'))?;
//                    }
//                } else {
//                    handle.write_str(SoftAsciiStr::from_unchecked(name.as_str()))?;
//                    handle.write_str(SoftAsciiStr::from_unchecked("*=utf8''"))?;
//                    let encoded = percent_encode_param_value(value.as_str());
//                    handle.write_str(&*encoded)?;
//                }
//            }
//        }
//    }
//
//    if !split_params.is_empty() {
//        for (name, parts) in split_params.iter_mut() {
//            let mut counter = 0;
//            while let Some(&(val, is_enc)) = parts.get(&counter) {
//                let val = val.as_str();
//                //TODO implement quoting/encoding of section parameters
//                if is_enc {
//                    if val.len() == 0 || !is_token(val) {
//                        bail!(InvalidMime(mime.to_string()));
//                    }
//                //FIXME as as_str is not the representation this will won't work
//                } else if val.starts_with(r#"""#) {
//                    if !is_quoted_string(val, mail_type) {
//                        bail!(InvalidMimeRq(mime.to_string()));
//                    }
//                } else {
//                    if !is_token(val) {
//                        bail!(InvalidMimeRq(mime.to_string()));
//                    }
//                }
//                counter += 1;
//            }
//
//            if counter as usize != parts.len() {
//                bail!(InvalidMime(mime.to_string()))
//            }
//
//            for (section, &(val, is_enc)) in parts.iter() {
//                handle.mark_fws_pos();
//                handle.write_char(SoftAsciiChar::from_unchecked(';'))?;
//                handle.mark_fws_pos();
//                handle.write_str(SoftAsciiStr::from_unchecked(name))?;
//                handle.write_char(SoftAsciiChar::from_unchecked('*'))?;
//                //OPTIMIZE (have 3 byte scretch memory as to_string 1. is ascii 2. len <= 3
//                handle.write_str(SoftAsciiStr::from_unchecked(&*section.to_string()))?;
//                if is_enc {
//                    handle.write_char(SoftAsciiChar::from_unchecked('*'))?;
//                }
//                handle.write_char(SoftAsciiChar::from_unchecked('='))?;
//                handle.write_str_unchecked(val.as_str())?;
//            }
//        }
//    }
//    Ok(())
//}
//
////FIXME we could use nom for it is's already imported anyway
//fn get_split_num<'a, 'b: 'a>(param_name: &'a EName<'b>) -> Result<(Option<(&'b str, u8)>, bool)> {
//    let param_name = param_name.as_str();
//    let mut iter = param_name.chars();
//    let mut last = iter.next_back();
//    let (end_idx, is_encoded) =
//        if Some('*') == last {
//            last = iter.next_back();
//            (param_name.len() - 1, true)
//        } else {
//            (param_name.len(), false)
//        };
//    let mut start_idx = end_idx;
//    while let Some(ch) = last {
//        //-=1 is ok as Mime already makes sure parameter names are ascii only
//        // even more we break on any non ascii chars anyway so even if wrong data
//        // is passed in this will not panic when slicing
//        start_idx -= 1;
//        if !ch.is_digit(10) {
//            if ch == '*' {
//                // do not include the section starting * e.g. abc*1* => (Some((abc,1)),true)
//                let actual_name = &param_name[..start_idx];
//                let section: u8 = param_name[start_idx+1..end_idx]
//                    .parse()
//                    //we now it's a number, so the only error can be Overflow
//                    .map_err(|_| error!(MimeSectionOverflow))?;
//
//                return Ok((Some((actual_name, section)), is_encoded));
//            } else {
//                return Ok((None, is_encoded));
//            }
//        }
//
//        last = iter.next_back();
//    }
//    return Ok((None, is_encoded))
//}

#[cfg(test)]
mod test {
    use super::*;

    ec_test! { writing_encoded, {
        MediaType::try_from("text/plain; arbitrary*=utf8''this%20is%it")?
    } => ascii => [
        Text "text/plain",
        MarkFWS,
        Text ";",
        MarkFWS,
        Text " arbitrary*=utf8''this%20is%it"
    ]}

    ec_test! { writing_normal, {
        MediaType::try_from("text/plain; a=abc")?
    } => ascii => [
        Text "text/plain",
        MarkFWS,
        Text ";",
        MarkFWS,
        Text " a=abc"
    ]}

    ec_test! { writing_needless_quoted, {
        MediaType::try_from("text/plain; a=\"abc\"")?
    } => ascii => [
        Text "text/plain",
        MarkFWS,
        Text ";",
        MarkFWS,
        Text " a=\"abc\""
    ]}

    ec_test! { writing_quoted, {
        MediaType::try_from("text/plain; a=\"abc def\"")?
    } => ascii => [
        Text "text/plain",
        MarkFWS,
        Text ";",
        MarkFWS,
        Text " a=\"abc def\""
    ]}

    ec_test! { writing_quoted_with_escape, {
        MediaType::try_from("text/plain; a=\"abc\\ def\"")?
    } => ascii => [
        Text "text/plain",
        MarkFWS,
        Text ";",
        MarkFWS,
        Text " a=\"abc\\ def\""
    ]}

    ec_test! { writing_quoted_utf8, {
        MediaType::try_from("text/plain; a=\"←→\"")?
    } => utf8 => [
        Text "text/plain",
        MarkFWS,
        Text ";",
        MarkFWS,
        Text " a=\"←→\""
    ]}

    ec_test! { #[ignore] writing_quoted_needed_encoding, {
        MediaType::try_from("text/plain; a=\"←→\"")?
    } => ascii => [
        Text "text/plain",
        MarkFWS,
        Text ";",
        MarkFWS,
        Text " a*=utf8\'\'%E2%86%90%E2%86%92"
    ]}

    ec_test! { writing_parts_simple, {
        MediaType::try_from("text/plain; a*0=abc; a*1=\" def\"")?
    } => ascii => [
        Text "text/plain",
        MarkFWS,
        Text ";",
        MarkFWS,
        Text " a*0=abc",
        MarkFWS,
        Text ";",
        MarkFWS,
        Text " a*1=\" def\""
    ]}

    //TODO media type needs parts awareness
    // i.e. currently it would do a*1=\"↓\"" => "a*1*=utf-8''%E2%86%93" which is wrong
    // as it's not the first part and it does not know about parts
    ec_test! { #[ignore] writing_parts_needs_encoding_not_first, {
        MediaType::try_from("text/plain; a*0=abc; a*1=\"↓\"")?
    } => ascii => [
        Text "text/plain",
        MarkFWS,
        Text ";",
        MarkFWS,
        Text " a*0*=utf8''abc",
        MarkFWS,
        Text ";",
        MarkFWS,
        Text " a*1*=%E2%86%93"
    ]}
}
