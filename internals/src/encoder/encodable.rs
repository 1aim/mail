use std::any::{Any, TypeId};
use std::fmt::{self, Debug};
use std::result::Result as StdResult;
use std::sync::Arc;

use super::EncodingWriter;
use error::EncodingError;

// can not be moved to `super::traits` as it depends on the
// EncodingWriter defined here
/// Trait Implemented by "components" used in header field bodies
///
/// This trait can be turned into a trait object allowing runtime
/// genericallity over the "components" if needed.
pub trait EncodableInHeader: Send + Sync + Any + Debug {
    fn encode(&self, encoder: &mut EncodingWriter) -> Result<(), EncodingError>;

    fn boxed_clone(&self) -> Box<EncodableInHeader>;

    #[doc(hidden)]
    fn type_id(&self) -> TypeId {
        TypeId::of::<Self>()
    }
}

//TODO we now could use MOPA or similar crates
impl EncodableInHeader {
    #[inline(always)]
    pub fn is<T: EncodableInHeader>(&self) -> bool {
        EncodableInHeader::type_id(self) == TypeId::of::<T>()
    }

    #[inline]
    pub fn downcast_ref<T: EncodableInHeader>(&self) -> Option<&T> {
        if self.is::<T>() {
            Some(unsafe { &*(self as *const EncodableInHeader as *const T) })
        } else {
            None
        }
    }

    #[inline]
    pub fn downcast_mut<T: EncodableInHeader>(&mut self) -> Option<&mut T> {
        if self.is::<T>() {
            Some(unsafe { &mut *(self as *mut EncodableInHeader as *mut T) })
        } else {
            None
        }
    }
}

impl Clone for Box<EncodableInHeader> {
    fn clone(&self) -> Self {
        self.boxed_clone()
    }
}

pub trait EncodableInHeaderBoxExt: Sized {
    fn downcast<T: EncodableInHeader>(self) -> StdResult<Box<T>, Self>;
}

impl EncodableInHeaderBoxExt for Box<EncodableInHeader> {
    fn downcast<T: EncodableInHeader>(self) -> StdResult<Box<T>, Self> {
        if EncodableInHeader::is::<T>(&*self) {
            let ptr: *mut EncodableInHeader = Box::into_raw(self);
            Ok(unsafe { Box::from_raw(ptr as *mut T) })
        } else {
            Err(self)
        }
    }
}

impl EncodableInHeaderBoxExt for Box<EncodableInHeader + Send> {
    fn downcast<T: EncodableInHeader>(self) -> StdResult<Box<T>, Self> {
        if EncodableInHeader::is::<T>(&*self) {
            let ptr: *mut EncodableInHeader = Box::into_raw(self);
            Ok(unsafe { Box::from_raw(ptr as *mut T) })
        } else {
            Err(self)
        }
    }
}

/// Generate a think implementing `EncodableInHeader` from an function.
///
/// (Mainly used in the inside of tests.)
#[macro_export]
macro_rules! enc_func {
    (|$enc:ident : &mut EncodingWriter| $block:block) => {{
        use $crate::error::EncodingError;
        fn _anonym($enc: &mut EncodingWriter) -> Result<(), EncodingError> {
            $block
        }
        let fn_pointer = _anonym as fn(&mut EncodingWriter) -> Result<(), EncodingError>;
        $crate::encoder::EncodeFn::new(fn_pointer)
    }};
}

type _EncodeFn = for<'a, 'b> fn(&'a mut EncodingWriter<'b>) -> Result<(), EncodingError>;

/// A wrapper for an function making it implement `EncodableInHeader`.
#[derive(Clone, Copy)]
pub struct EncodeFn(_EncodeFn);

impl EncodeFn {
    pub fn new(func: _EncodeFn) -> Self {
        EncodeFn(func)
    }
}

impl EncodableInHeader for EncodeFn {
    fn encode(&self, encoder: &mut EncodingWriter) -> Result<(), EncodingError> {
        (self.0)(encoder)
    }

    fn boxed_clone(&self) -> Box<EncodableInHeader> {
        Box::new(*self)
    }
}

impl Debug for EncodeFn {
    fn fmt(&self, fter: &mut fmt::Formatter) -> fmt::Result {
        write!(fter, "EncodeFn(..)")
    }
}

/// Generate a think implementing `EncodableInHeader` from an closure.
///
/// (Mainly used in the inside of tests.)
#[macro_export]
macro_rules! enc_closure {
    ($($t:tt)*) => ({
        $crate::encoder::EncodeClosure::new($($t)*)
    });
}

/// A wrapper for an closure making it implement `EncodableInHeader`.
pub struct EncodeClosure<FN: 'static>(Arc<FN>)
where
    FN: Send + Sync + for<'a, 'b> Fn(&'a mut EncodingWriter<'b>) -> Result<(), EncodingError>;

impl<FN: 'static> EncodeClosure<FN>
where
    FN: Send + Sync + for<'a, 'b> Fn(&'a mut EncodingWriter<'b>) -> Result<(), EncodingError>,
{
    pub fn new(closure: FN) -> Self {
        EncodeClosure(Arc::new(closure))
    }
}

impl<FN: 'static> EncodableInHeader for EncodeClosure<FN>
where
    FN: Send + Sync + for<'a, 'b> Fn(&'a mut EncodingWriter<'b>) -> Result<(), EncodingError>,
{
    fn encode(&self, encoder: &mut EncodingWriter) -> Result<(), EncodingError> {
        (self.0)(encoder)
    }

    fn boxed_clone(&self) -> Box<EncodableInHeader> {
        Box::new(self.clone())
    }
}

impl<FN: 'static> Clone for EncodeClosure<FN>
where
    FN: Send + Sync + for<'a, 'b> Fn(&'a mut EncodingWriter<'b>) -> Result<(), EncodingError>,
{
    fn clone(&self) -> Self {
        EncodeClosure(self.0.clone())
    }
}

impl<FN: 'static> Debug for EncodeClosure<FN>
where
    FN: Send + Sync + for<'a, 'b> Fn(&'a mut EncodingWriter<'b>) -> Result<(), EncodingError>,
{
    fn fmt(&self, fter: &mut fmt::Formatter) -> fmt::Result {
        write!(fter, "EncodeClosure(..)")
    }
}
