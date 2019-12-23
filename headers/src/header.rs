use std::any::TypeId;
use std::fmt::{self, Debug};
use std::ops::{Deref, DerefMut};

use internals::{
    encoder::{EncodableInHeader, EncodingWriter},
    error::EncodingError,
};

use convert::HeaderTryInto;
use error::ComponentCreationError;
use name::{HasHeaderName, HeaderName};
//NOTE: this is a circular dependency between Header/HeaderMap
// but putting up e.g. a GenericHeaderMap trait/interface is
// not worth the work at all
use map::HeaderMapValidator;

/// Trait representing a mail header.
///
/// **This is not meant to be implemented by hand.***
/// Use the `def_headers` macro instead.
///
pub trait HeaderKind: Clone + Default + 'static {
    /// the component representing the header-field, e.g. `Unstructured` for `Subject`
    type Component: EncodableInHeader + Clone;

    //FIXME[rust/const fn]: make this a associated constant
    /// a method returning the header name
    ///
    /// # Note:
    /// Once `const fn` is stable this will be changed to
    /// a associated constant.
    fn name() -> HeaderName;

    /// A function which is meant to be called with a reference
    /// to the final header map before encoding the headers. It is
    /// meant to be used do some of the contextual validations,
    /// like e.g. a `From` header might return a function which
    /// checks if the `From` header has multiple mailboxes and
    /// if so checks if there is a `Sender` header
    ///
    /// Calling a contextual validator with a header map not
    /// containing a header which it is meant to validate
    /// should not cause an error. Only if the header is
    /// there and the component is of the expected type
    /// and it is invalid in the context
    /// an error should be returned.
    const VALIDATOR: Option<HeaderMapValidator>;

    /// I true this will assure that the header is at most one time in a header map.
    ///
    /// This is similar to `VALIDATOR` (and can be archived through one) but in difference
    /// to any `VALIDATOR` this is already assured when inserting a header with MAX_ONE set
    /// to true in a header map. It exists so that the header map can do, what is most
    /// intuitive, replacing insertion for all `MAX_ONE` headers (like in a normal map) but
    /// use adding insertion for all other header (like in a multi map).
    ///
    /// Most headers have this set to true.
    const MAX_ONE: bool;

    /// Creates a `Header` instance automatically converting given body to the right type.
    ///
    /// # Error
    ///
    /// The type system assure that you can only use it on conversions
    /// which are possible on type level, but they can still fail depending
    /// on the actual data. For example creating a `Email`  from a string
    /// can fail if the string is not a valid email address. This in
    /// turn means that creating a `From` header from a array of strings
    /// can fail if one of them is not a valid email address.
    fn auto_body<H>(body: H) -> Result<Header<Self>, ComponentCreationError>
    where
        H: HeaderTryInto<Self::Component>,
    {
        Ok(Self::body(HeaderTryInto::try_into(body)?))
    }

    /// Creates a `Header` instance for this header kind with given body.
    fn body(body: Self::Component) -> Header<Self> {
        Header::new(body)
    }
}

impl<H> HasHeaderName for H
where
    H: HeaderKind,
{
    fn get_name(&self) -> HeaderName {
        H::name()
    }
}

pub trait MaxOneMarker: HeaderKind {}

#[derive(Clone)]
pub struct Header<H>
where
    H: HeaderKind,
{
    body: H::Component,
}

impl<H> Header<H>
where
    H: HeaderKind,
{
    pub fn new(body: H::Component) -> Header<H> {
        Header { body }
    }

    pub fn body(&self) -> &H::Component {
        &self.body
    }

    pub fn body_mut(&mut self) -> &mut H::Component {
        &mut self.body
    }
}

impl<H> Deref for Header<H>
where
    H: HeaderKind,
{
    type Target = H::Component;
    fn deref(&self) -> &Self::Target {
        self.body()
    }
}

impl<H> DerefMut for Header<H>
where
    H: HeaderKind,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.body_mut()
    }
}

impl<H> Debug for Header<H>
where
    H: HeaderKind,
{
    fn fmt(&self, fter: &mut fmt::Formatter) -> fmt::Result {
        self.body.fmt(fter)
    }
}

/// Type alias for HeaderObjTrait's trait object.
pub type HeaderObj = dyn HeaderObjTrait;

pub trait HeaderObjTrait: Sync + Send + ::std::any::Any + Debug {
    fn name(&self) -> HeaderName;
    fn is_max_one(&self) -> bool;
    fn validator(&self) -> Option<HeaderMapValidator>;
    fn encode(&self, encoder: &mut EncodingWriter) -> Result<(), EncodingError>;
    fn boxed_clone(&self) -> Box<HeaderObj>;

    #[doc(hidden)]
    fn type_id(&self) -> TypeId {
        TypeId::of::<Self>()
    }
}

impl<H> HeaderObjTrait for Header<H>
where
    H: HeaderKind,
{
    fn name(&self) -> HeaderName {
        H::name()
    }

    fn is_max_one(&self) -> bool {
        H::MAX_ONE
    }

    fn validator(&self) -> Option<HeaderMapValidator> {
        H::VALIDATOR
    }

    fn encode(&self, encoder: &mut EncodingWriter) -> Result<(), EncodingError> {
        self.body.encode(encoder)
    }

    fn boxed_clone(&self) -> Box<HeaderObj> {
        let cloned = self.clone();
        Box::new(cloned)
    }
}

impl<H> HasHeaderName for Header<H>
where
    H: HeaderKind,
{
    fn get_name(&self) -> HeaderName {
        H::name()
    }
}

impl HeaderObj {
    pub fn is<H>(&self) -> bool
    where
        H: HeaderKind,
    {
        HeaderObjTrait::type_id(self) == TypeId::of::<Header<H>>()
    }

    pub fn downcast_ref<H>(&self) -> Option<&Header<H>>
    where
        H: HeaderKind,
    {
        if self.is::<H>() {
            Some(unsafe { &*(self as *const _ as *const Header<H>) })
        } else {
            None
        }
    }

    pub fn downcast_mut<H>(&mut self) -> Option<&mut Header<H>>
    where
        H: HeaderKind,
    {
        if self.is::<H>() {
            Some(unsafe { &mut *(self as *mut _ as *mut Header<H>) })
        } else {
            None
        }
    }
}

impl Clone for Box<HeaderObj> {
    fn clone(&self) -> Self {
        self.boxed_clone()
    }
}

impl HasHeaderName for HeaderObj {
    fn get_name(&self) -> HeaderName {
        self.name()
    }
}

pub trait HeaderObjTraitBoxExt: Sized {
    fn downcast<H>(self) -> Result<Box<Header<H>>, Self>
    where
        H: HeaderKind;
}

impl HeaderObjTraitBoxExt for Box<HeaderObjTrait> {
    fn downcast<H>(self) -> Result<Box<Header<H>>, Self>
    where
        H: HeaderKind,
    {
        if HeaderObjTrait::is::<H>(&*self) {
            let ptr: *mut (HeaderObj) = Box::into_raw(self);
            Ok(unsafe { Box::from_raw(ptr as *mut Header<H>) })
        } else {
            Err(self)
        }
    }
}
