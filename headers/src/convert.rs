//TODO potentially move HeaderTryFrom to `mail-headers`
use error::ComponentCreationError;

//TODO replace with std TryFrom once it is stable
// (either a hard replace, or a soft replace which implements HeaderTryFrom if TryFrom exist)
/// Workaround for `TryFrom`,`TryInto` not being stable.
pub trait HeaderTryFrom<T>: Sized {
    fn try_from(val: T) -> Result<Self, ComponentCreationError>;
}

/// Workaround for `TryFrom`,`TryInto` not being stable.
pub trait HeaderTryInto<T>: Sized {
    fn try_into(self) -> Result<T, ComponentCreationError>;
}

impl<F, T> HeaderTryInto<T> for F
where
    T: HeaderTryFrom<F>,
{
    fn try_into(self) -> Result<T, ComponentCreationError> {
        T::try_from(self)
    }
}

impl<T> HeaderTryFrom<T> for T {
    fn try_from(val: T) -> Result<Self, ComponentCreationError> {
        Ok(val)
    }
}

// It is not possible to auto-implement HeaderTryFrom for From/Into as
// this will make new HeaderTryFrom implementations outside of this care
// nearly impossible making the trait partially useless
//
//impl<T, F> HeaderTryFrom<F> for T where F: Into<T> {
//    fn try_from(val: F) -> Result<T, Error> {
//        Ok( val.into() )
//    }
//}
