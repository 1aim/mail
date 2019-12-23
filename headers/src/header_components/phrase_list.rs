use soft_ascii_string::SoftAsciiChar;

use vec1::{Size0Error, Vec1};

use error::ComponentCreationError;
use internals::encoder::{EncodableInHeader, EncodingWriter};
use internals::error::EncodingError;
use {HeaderTryFrom, HeaderTryInto};

use super::Phrase;

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct PhraseList(pub Vec1<Phrase>);

impl IntoIterator for PhraseList {
    type Item = <Vec1<Phrase> as IntoIterator>::Item;
    type IntoIter = <Vec1<Phrase> as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl EncodableInHeader for PhraseList {
    fn encode(&self, handle: &mut EncodingWriter) -> Result<(), EncodingError> {
        sep_for! { word in self.0.iter();
            sep {
                //TODO handle this better by collapsing FWS
                // <= isn't that allready fixed by FWS+ has content on line in EncodingBuffer
                //Note that we do not want to write FWS as the following word might contains
                // a left_padding with a MarkFWS, NowChar, Text " " but a space if fine
                handle.write_char( SoftAsciiChar::from_unchecked(',') )?;
                handle.write_char( SoftAsciiChar::from_unchecked(' ') )?;
            };
            word.encode( handle )?;

        }

        Ok(())
    }

    fn boxed_clone(&self) -> Box<EncodableInHeader> {
        Box::new(self.clone())
    }
}

impl<T> HeaderTryFrom<T> for PhraseList
where
    T: HeaderTryInto<Phrase>,
{
    fn try_from(phrase: T) -> Result<Self, ComponentCreationError> {
        let phrase = phrase.try_into()?;
        Ok(PhraseList(Vec1::new(phrase)))
    }
}

impl<T> HeaderTryFrom<Vec<T>> for PhraseList
where
    T: HeaderTryInto<Phrase>,
{
    fn try_from(vec: Vec<T>) -> Result<Self, ComponentCreationError> {
        try_from_into_iter(vec)
    }
}

fn try_from_into_iter<IT>(phrases: IT) -> Result<PhraseList, ComponentCreationError>
where
    IT: IntoIterator,
    IT::Item: HeaderTryInto<Phrase>,
{
    let mut iter = phrases.into_iter();
    let mut vec = if let Some(first) = iter.next() {
        Vec1::new(first.try_into()?)
    } else {
        return Err(ComponentCreationError::from_parent(
            Size0Error,
            "PhraseList",
        ));
    };
    for phrase in iter {
        vec.push(phrase.try_into()?);
    }
    Ok(PhraseList(vec))
}

//FIXME: dedup code duplication with:
// MailboxList, PhraseList(this think here) and ?? possible future types??
macro_rules! impl_header_try_from_array {
    (_MBoxList 0) => ();
    (_MBoxList $len:tt) => (
        impl<T> HeaderTryFrom<[T; $len]> for PhraseList
            where T: HeaderTryInto<Phrase>
        {
            fn try_from( vec: [T; $len] ) -> Result<Self, ComponentCreationError> {
                //due to only supporting arrays halfheartedly for now
                let heapified: Box<[T]> = Box::new(vec);
                let vecified: Vec<_> = heapified.into();
                try_from_into_iter( vecified )
            }
        }
    );
    ($($len:tt)*) => ($(
        impl_header_try_from_array!{ _MBoxList $len }
    )*);
}

impl_header_try_from_array! {
     0  1  2  3  4  5  6  7  8  9
    10 11 12 13 14 15 16 17 18 19
    20 21 22 23 24 25 26 27 28 29
    30 31 32
}

#[cfg(test)]
mod test {
    use super::*;

    ec_test! { some_phrases, {
        PhraseList( vec1![
            Phrase::try_from( "hy there" )?,
            Phrase::try_from( "magic man" )?
        ])
    } => ascii => [
        Text "hy",
        MarkFWS,
        //TODO really no FWS by the seperator??
        // (currently it's this way as word can start with a FWS making it a double FWS)
        Text " there, magic",
        MarkFWS,
        Text " man"
    ]}

    ec_test! { some_simple_phrases_try_from, {
        PhraseList::try_from(
            "hy there"
        )?
    } => ascii => [
        Text "hy",
        MarkFWS,
        Text " there"
    ]}

    ec_test! { some_phrases_try_from, {
        PhraseList::try_from( [
            "hy there",
            "magic man"
        ] )?
    } => ascii => [
        Text "hy",
        MarkFWS,
        Text " there, magic",
        MarkFWS,
        Text " man"
    ]}
}
