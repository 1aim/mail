use vec1::{Size0Error, Vec1};

use internals::encoder::{EncodableInHeader, EncodingWriter};
use internals::error::EncodingError;
use internals::grammar::encoded_word::EncodedWordContext;

use data::Input;
use error::ComponentCreationError;
use {HeaderTryFrom, HeaderTryInto};

use super::utils::text_partition::{partition, Partition};
use super::word::{do_encode_word, Word};
use super::{CFWS, FWS};

/// Represent a "phrase" as it for example is used in the `Mailbox` type for the display name.
///
/// It is recommended to use the [`Phrase.new()`] constructor, which creates the right phrase
/// for your input.
///
/// **Warning: Details of this type, expect `Phrase::new` and `Phrase::try_from`, are likely to
///   change with some of the coming braking changes.** If you just create it using `try_from`
///   or `new` changes should not affect you, but if you create it from a vec of `Word`'s things
///   might be different.
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct Phrase(
    //FIXME hide this away or at last turn it into a struct field, with next braking change.
    /// The "words" the phrase consist of. Be aware that this are words in the sense of the
    /// mail grammar so it can be a complete quoted string. Also be aware that in the mail
    /// grammar "words" _contain the whitespace around them_ (to some degree). So if you
    /// just have a sequence of "human words"  turned into word instances there will be
    /// no whitespace between the words. (From the point of the mail grammar a words do not
    /// have to have any boundaries between each other even if this leads to ambiguity)
    pub Vec1<Word>,
);

impl Phrase {
    /// Creates a `Phrase` instance from some arbitrary input.
    ///
    /// This method can be used with both `&str` and `String`.
    ///
    /// # Error
    ///
    /// There are only two cases in which this can fail:
    ///
    /// 1. If the input is empty (a phrase can not be empty).
    /// 2. If the input contained a illegal us-ascii character (any char which is
    ///    not "visible" and not `' '` or `\t` like e.g. CTRL chars `'\0'` but also
    ///    `'\r'` and `'\n'`). While we could encode them with encoded words, it's
    ///    not really meant to be used this way and this chars will likely either be
    ///    stripped out by a mail client or might cause display bugs.
    pub fn new<T: HeaderTryInto<Input>>(input: T) -> Result<Self, ComponentCreationError> {
        //TODO it would make much more sense if Input::shared could be taken advantage of
        let input = input.try_into()?;

        //OPTIMIZE: words => shared, then turn partition into shares, too
        let mut last_gap = None;
        let mut words = Vec::new();
        let partitions = partition(input.as_str()).map_err(|err| {
            ComponentCreationError::from_parent(err, "Phrase").with_str_context(input.as_str())
        })?;

        for partition in partitions.into_iter() {
            match partition {
                Partition::VCHAR(word) => {
                    let mut word = Word::try_from(word)?;
                    if let Some(fws) = last_gap.take() {
                        word.pad_left(fws);
                    }
                    words.push(word);
                }
                Partition::SPACE(_gap) => {
                    //FIMXE currently collapses WS (This will leave at last one WS!)
                    last_gap = Some(CFWS::SingleFws(FWS))
                }
            }
        }

        let mut words = Vec1::try_from_vec(words).map_err(|_| {
            ComponentCreationError::from_parent(Size0Error, "Phrase")
                .with_str_context(input.as_str())
        })?;

        if let Some(right_padding) = last_gap {
            words.last_mut().pad_right(right_padding);
        }

        Ok(Phrase(words))
    }
}

impl<'a> HeaderTryFrom<&'a str> for Phrase {
    fn try_from(input: &'a str) -> Result<Self, ComponentCreationError> {
        Phrase::new(input)
    }
}

impl HeaderTryFrom<String> for Phrase {
    fn try_from(input: String) -> Result<Self, ComponentCreationError> {
        Phrase::new(input)
    }
}

impl HeaderTryFrom<Input> for Phrase {
    fn try_from(input: Input) -> Result<Self, ComponentCreationError> {
        Phrase::new(input)
    }
}

impl EncodableInHeader for Phrase {
    //FEATURE_TODO(warn_on_bad_phrase): warn if the phrase contains chars it should not
    //  but can contain due to encoding, e.g. ascii CTL's
    fn encode(&self, heandle: &mut EncodingWriter) -> Result<(), EncodingError> {
        for word in self.0.iter() {
            do_encode_word(&*word, heandle, Some(EncodedWordContext::Phrase))?;
        }

        Ok(())
    }

    fn boxed_clone(&self) -> Box<EncodableInHeader> {
        Box::new(self.clone())
    }
}

#[cfg(test)]
mod test {
    use super::Phrase;
    use HeaderTryFrom;

    ec_test! { simple, {
        Phrase::try_from("simple think")?
    } => ascii => [
        Text "simple",
        MarkFWS,
        Text " think"
    ]}

    ec_test! { with_encoding, {
        Phrase::try_from(" hm nääds encoding")?
    } => ascii => [
        MarkFWS,
        Text " hm",
        MarkFWS,
        Text " =?utf8?Q?n=C3=A4=C3=A4ds?=",
        MarkFWS,
        Text " encoding"
    ]}
}
