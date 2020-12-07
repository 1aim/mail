//! Module containing the `HeaderMap`.
//!
//! It also contains some helper types like iterator types
//! for the HeaderMap etc.
use std::cmp::PartialEq;
use std::collections::HashSet;
use std::fmt::{self, Debug};
use std::hash::{Hash, Hasher};
use std::iter::ExactSizeIterator;
use std::marker::PhantomData;

use total_order_multi_map::{self, EntryValues, EntryValuesMut, TotalOrderMultiMap};

use error::{BuildInValidationError, HeaderTypeError, HeaderValidationError};

use name::{HasHeaderName, HeaderName};

use header::{Header, HeaderKind, HeaderObj, HeaderObjTrait, MaxOneMarker};

mod into_iter;
pub use self::into_iter::*;

/// The type of an validator used to check more complex header contraints.
///
/// An example constraint would be if a `From` header field contains more than
/// one mailbox a `Sender` header field is required to be present.
pub type HeaderMapValidator = fn(&HeaderMap) -> Result<(), ::error::HeaderValidationError>;

//TODO extend example to use get,get_mut etc.
/// A header map is a collection representing a number
/// of mail headers in an specific order.
///
///
/// # Example
///
/// ```
/// # #[macro_use]
/// # extern crate mail_headers;
///
/// // just import all headers
/// use mail_headers::HeaderMap;
/// use mail_headers::headers::*;
/// use mail_headers::error::ComponentCreationError;
///
/// fn create_headers() -> Result<HeaderMap, ComponentCreationError> {
///     headers!{
///         // from and to can have multiple values
///         // until specialization is stable is array
///         // is necessary
///         _From: [("My Fancy Display Name", "theduck@example.com")],
///         _To: [ "unknown@example.com", ],
///         Subject: "Who are you?"
///     }
/// }
///
/// fn main() {
///     let headers = create_headers().unwrap();
///     assert_eq!(headers.len(), 3);
/// }
/// ```
///
/// # Note
///
/// A number of methods implemented on HeaderMap appear in two variations,
/// one which accepts a type hint (a normally zero sized struct implementing
/// HeaderKind) and on which just accepts the type and needs to be called with
/// the turbofish operator. The later one is prefixed by a `_` as the former
/// one is more nice to use, but in some situations, e.g. when wrapping
/// `HeaderMap` in custom code the only type accepting variations are more
/// useful.
///
/// ```rust,ignore
/// let _ = map.get(Subject);
/// //is equivalent to
/// let _ = map._get::<Subject>();
/// ```
///
/// # MaxOne (In-)Consistency
///
/// Most headers can only appear up to one time in a header section.
/// They are marked with `H::MAX_ONE == true` and implement `MaxOneMarker`,
/// also as object you can use `is_max_one` to check it.
///
/// Not only can they only appear max one time, it is normal for a user
/// who is not aware about the other headers to expect that when you insert
/// them into a header map which already contains them that they replace
/// the existing header. Even more so most headers which can appear more
/// then one time are unlikely to appear in a application of this library
/// like e.g. all `Resent-*` header which normally get just prepended
/// to existing mail in text format or the `Comment` header which isn't
/// used that much.
///
/// Because of this it was decided that when inserting a `"max one"` header
/// it will act as expected an replace other headers with the same name and
/// only if a `"multi"` header is inserted it is added to all headers associated
/// with the same name.
///
/// But there is a single problem. If there are multiple implementations implementations
/// for the same header which disagree in wether or not the header is `"max one"` (which
/// would be a bug anyway!) then this can lead to a tricky situration when you first
/// insert the version which is `"max one"` and then the one which is `"multi"`.
/// There had been two ways to deal with this:
///
/// 1. return a error when inserting in such a situation
/// 2. simple allow it and check it when running the other
///    validators
///
/// Given that a header map contains additionally validators which needs
/// to be run explicitly to make sure that a map is valid before using it
/// as a header section in a mail it was decided to go with the later approach.
/// Originally the first approach was implemented but turned out to be not
/// very ergonomic, and the second approach has little disadvantages as:
///
/// - it's already unlikely to run into the situation
/// - you have to run validators anyway before using the
///   header map
///
///
/// **So yes, you can not relay on the "max one" constraints
///   to be uphold without running the validators**
///
///
#[derive(Clone)]
pub struct HeaderMap {
    inner_map: TotalOrderMultiMap<HeaderName, Box<HeaderObj>>,
}

pub type Iter<'a> = total_order_multi_map::Iter<'a, HeaderName, Box<HeaderObj>>;
pub type IterMut<'a> = total_order_multi_map::IterMut<'a, HeaderName, Box<HeaderObj>>;
pub type Values<'a> = total_order_multi_map::Values<'a, HeaderName, Box<HeaderObj>>;
pub type ValuesMut<'a> = total_order_multi_map::ValuesMut<'a, HeaderName, Box<HeaderObj>>;

impl Debug for HeaderMap {
    fn fmt(&self, fter: &mut fmt::Formatter) -> fmt::Result {
        write!(fter, "HeaderMap {{ ")?;
        for (key, val_cont) in self.iter() {
            write!(fter, "{}: {:?},", key.as_str(), val_cont)?;
        }
        write!(fter, " }}")
    }
}

impl Default for HeaderMap {
    fn default() -> Self {
        HeaderMap {
            inner_map: Default::default(),
        }
    }
}

impl HeaderMap {
    /// create a new empty header map
    pub fn new() -> Self {
        Default::default()
    }

    /// returns the number of headers in this map
    pub fn len(&self) -> usize {
        self.inner_map.len()
    }

    /// clears the header map
    ///
    /// This removes all headers _and_ all validators
    pub fn clear(&mut self) {
        self.inner_map.clear();
    }

    /// Iterate over all `HeaderObj` added to the map.
    pub fn values(&self) -> Values {
        self.inner_map.values()
    }

    /// Iterate with mut refs over all `HeaderObj` added to the map.
    pub fn values_mut(&mut self) -> ValuesMut {
        self.inner_map.values_mut()
    }

    /// call each unique contextual validator exactly once with this map as parameter
    ///
    /// If multiple Headers provide the same contextual validator (e.g. the resent headers)
    /// it's still only called once.
    ///
    /// # Max One
    ///
    /// This will also validate that for any header name for which a header
    /// was added with `MAX_ONE == true` it will be validated that it is the
    /// only header for that header name.
    pub fn use_contextual_validators(&self) -> Result<(), HeaderValidationError> {
        let mut seen_validators = HashSet::new();

        let mut validate = |validator| -> Result<(), HeaderValidationError> {
            if let Some(validator) = validator {
                if seen_validators.insert(ValidatorHashWrapper(validator)) {
                    (validator)(self)?;
                }
            }
            Ok(())
        };

        for mut group in self.inner_map.group_iter() {
            let first = group
                .next()
                .expect("[BUG] returned header without any headers inserted for it");
            let max_one = first.is_max_one();
            validate(first.validator())?;
            let header_name = group.key().as_str();
            for other in group {
                if max_one != other.is_max_one() {
                    return Err(BuildInValidationError::MaxOneInconsistency { header_name }.into());
                }
                validate(other.validator())?;
            }
        }
        Ok(())
    }

    /// Returns true if this map contains a header with the given name.
    pub fn contains<H: HasHeaderName>(&self, name: H) -> bool {
        self.inner_map.contains_key(name.get_name())
    }

    /// Returns the single header associated with the given header kind.
    ///
    /// As this uses the `MaxOneMarker` trait which _should_ only be implemented
    /// for `HeaderKind` impl with `MAX_ONE == true` this function can only
    /// be used when it's fine to ignore the possible case of more than
    /// one header of the given kind being in the same map.
    ///
    /// # Type Hint
    ///
    /// The type hint passed in is for ergonomics, e.g. so
    /// that it's possible to write code like `map.get_single(Subject)`
    /// if this gets in the way `_get_single` can be used which would
    /// lead to code like `map._get_single::<Subject>()`.
    ///
    /// # Error
    ///
    /// - If there are multiple implementations for the same header and
    ///   the inserted headers is based on a different type some `HeaderTypeError`
    ///   is returned
    ///
    /// - If there are multiple implementations for the same header which
    ///   disagree on the value of `H::MAX_ONE` (which is a bug) this can
    ///   in some rare situations lead to be there more then one header for
    ///   a "max one" header in the map, in which case a `HeaderTypeError`
    ///   is returned.
    #[inline]
    pub fn get_single<'a, H>(
        &'a self,
        _type_hint: H,
    ) -> Option<Result<&'a Header<H>, HeaderTypeError>>
    where
        H: MaxOneMarker,
    {
        self._get_single::<H>()
    }

    /// A variation of `get_single` which doesn't require passing in a type hint.
    ///
    /// Normally using `get_single` is more ergonomic, except if you write a function
    /// which abstracts over it in which case using `_get_single` can be better.
    pub fn _get_single<'a, H>(&'a self) -> Option<Result<&'a Header<H>, HeaderTypeError>>
    where
        H: MaxOneMarker,
    {
        let mut bodies = self.get_untyped(H::name());
        if bodies.len() > 1 {
            return Some(Err(HeaderTypeError::new(H::name())));
        }

        bodies.next().map(|untyped| {
            untyped
                .downcast_ref::<H>()
                .ok_or_else(|| HeaderTypeError::new(H::name()))
        })
    }

    /// Returns a a mutable reference to the header associated with the given header kind.__internals
    ///
    /// See `HeaderMap::get_single` for more details.
    #[inline]
    pub fn get_single_mut<H>(
        &mut self,
        _type_hint: H,
    ) -> Option<Result<&mut Header<H>, HeaderTypeError>>
    where
        H: MaxOneMarker,
    {
        self._get_single_mut::<H>()
    }

    /// Returns a a mutable reference to the header associated with the given header kind.__internals
    ///
    /// See `HeaderMap::_get_single` for more details.
    pub fn _get_single_mut<H>(&mut self) -> Option<Result<&mut Header<H>, HeaderTypeError>>
    where
        H: MaxOneMarker,
    {
        let mut bodies = self.get_untyped_mut(H::name());
        if bodies.len() > 1 {
            return Some(Err(HeaderTypeError::new(H::name())));
        }

        bodies.next().map(|untyped| {
            untyped
                .downcast_mut::<H>()
                .ok_or_else(|| HeaderTypeError::new(H::name()))
        })
    }

    /// Returns all header bodies for a given header name, without trying to cast them to a concrete type
    ///
    /// Accepts both `HeaderName` or a type implementing `HeaderKind`.
    ///
    #[inline]
    pub fn get_untyped<H: HasHeaderName>(&self, name: H) -> UntypedBodies {
        self.inner_map.get(name.get_name())
    }

    /// Returns all header bodies for a given header name, without trying to cast them to a concrete type
    ///
    /// Accepts both `HeaderName` or a type implementing `HeaderKind`.
    ///
    #[inline]
    pub fn get_untyped_mut<H: HasHeaderName>(&mut self, name: H) -> UntypedBodiesMut {
        self.inner_map.get_mut(name.get_name())
    }

    /// Returns all header bodies for a given header
    #[inline(always)]
    pub fn get<H>(&self, _type_hint: H) -> TypedBodies<H>
    where
        H: HeaderKind,
    {
        self._get::<H>()
    }

    /// Returns all header bodies for a given header
    pub fn _get<H>(&self) -> TypedBodies<H>
    where
        H: HeaderKind,
    {
        self.get_untyped(H::name()).into()
    }

    /// Returns all header bodies for a given header
    #[inline(always)]
    pub fn get_mut<H>(&mut self, _type_hint: H) -> TypedBodiesMut<H>
    where
        H: HeaderKind,
    {
        self._get_mut::<H>()
    }

    /// Returns all header bodies for a given header
    pub fn _get_mut<H>(&mut self) -> TypedBodiesMut<H>
    where
        H: HeaderKind,
    {
        self.get_untyped_mut(H::name()).into()
    }

    /// Inserts the given header into the map either replacing or adding to existing headers.
    ///
    /// - If `H::MAX_ONE` is `true` then it will use "replacing insert" which means
    ///   all headers previously associated with the given  header (name) are removed when
    ///   adding the new header.
    ///
    ///   This behavior is analog to how a normal map works and
    ///   is what a user which isn't aware that there are some headers which can appear multiple
    ///   times would expect. Most common headers (`Subject`, `From`, `To`, `Sender`, etc.) fall
    ///   into this category.
    ///
    /// - If `H::MAX_ONE` is `false` then it will use "adding insert" which means
    ///   that it will add the header to all headers previously associated with the given
    ///   header name.
    ///
    pub fn insert<H>(&mut self, header: Header<H>)
    where
        H: HeaderKind,
    {
        let name = header.name();
        let obj: Box<HeaderObj> = Box::new(header);
        self._insert(name, H::MAX_ONE, obj)
    }

    /// Insert a HeaderObj into the header map.
    #[doc(hidden)]
    pub fn insert_untyped(&mut self, obj: Box<HeaderObj>) {
        self._insert(obj.name(), obj.is_max_one(), obj)
    }

    #[inline(always)]
    fn _insert(&mut self, name: HeaderName, max_one: bool, obj: Box<HeaderObj>) {
        if max_one {
            self.inner_map.set(name, obj);
        } else {
            self.inner_map.add(name, obj);
        }
    }

    /// Insert all given headers in order into this header map.
    ///
    /// The insertion order of the given headers into this map
    /// is the same as the order in which they had been inserted
    /// into the header map through which they had been given to
    /// this method.
    ///
    /// As this uses insertion it also means that headers with
    /// `MAX_ONE == true` in the headers to insert can replace
    /// existing headers associated with the same header name.
    ///
    /// # Example
    ///
    /// ```
    /// # #[macro_use]
    /// # extern crate mail_headers;
    /// # fn main() {
    /// use mail_headers::headers::*;
    ///
    /// let mut map = headers!{
    ///     _From: [("Not Met", "it.s.me@example.com")],
    ///     Subject: "..."
    /// }.unwrap();
    ///
    /// map.insert_all(headers! {
    ///     _To: [("You", "someone@example.com")],
    ///     Subject: "expected subject"
    /// }.unwrap());
    ///
    /// assert_eq!(map.len(), 3);
    /// let subject = map.get_single(Subject)
    ///     .expect("Subject to be in map (Some)")
    ///     .expect("The type to be correct (Ok)");
    ///
    /// assert_eq!(subject.as_str(), "expected subject");
    /// assert!(map.contains(_From));
    /// assert!(map.contains(_To));
    /// # }
    /// ```
    ///
    pub fn insert_all(&mut self, other: HeaderMap) {
        for (_name, header) in other.into_iter() {
            self.insert_untyped(header);
        }
    }

    /// Remove all headers with the given header name.
    ///
    /// Returns true, if at last one header was removed.
    pub fn remove<H: HasHeaderName>(&mut self, name: H) -> bool {
        self.inner_map.remove_all(name.get_name())
    }

    /// iterate over all (header name, boxed body) pairs in this map
    pub fn iter(&self) -> Iter {
        self.inner_map.iter()
    }
}

/// Iterator over all boxed bodies for a given header name
pub type UntypedBodies<'a> = EntryValues<'a, HeaderObj>;
pub type UntypedBodiesMut<'a> = EntryValuesMut<'a, HeaderObj>;

/// Iterator over all boxed bodies for a given header name with knows which type they should have
///
/// This iterator will automatically try to cast each header body of this
/// header to `H::Component`, i.e. the type this body _should_ have.
pub struct TypedBodies<'a, H>
where
    H: HeaderKind,
{
    inner: UntypedBodies<'a>,
    _marker: PhantomData<H>,
}

impl<'a, H> From<UntypedBodies<'a>> for TypedBodies<'a, H>
where
    H: HeaderKind,
{
    fn from(untyped: UntypedBodies<'a>) -> Self {
        Self::new(untyped)
    }
}

impl<'a, H> TypedBodies<'a, H>
where
    H: HeaderKind,
{
    fn new(inner: UntypedBodies<'a>) -> Self {
        TypedBodies {
            inner,
            _marker: PhantomData,
        }
    }
}

impl<'a, H> Iterator for TypedBodies<'a, H>
where
    H: HeaderKind,
{
    type Item = Result<&'a Header<H>, HeaderTypeError>;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|tobj| {
            tobj.downcast_ref::<H>()
                .ok_or_else(|| HeaderTypeError::new(H::name()))
        })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl<'a, H> ExactSizeIterator for TypedBodies<'a, H>
where
    H: HeaderKind,
{
    fn len(&self) -> usize {
        self.inner.len()
    }
}

impl<'a, H> Clone for TypedBodies<'a, H>
where
    H: HeaderKind,
{
    fn clone(&self) -> Self {
        TypedBodies::new(self.inner.clone())
    }
}

impl<'a, H> Debug for TypedBodies<'a, H>
where
    H: HeaderKind,
{
    fn fmt(&self, fter: &mut fmt::Formatter) -> fmt::Result {
        fter.debug_struct("TypedBodies")
            .field("inner", &self.inner)
            .finish()
    }
}

/// Iterator over all boxed bodies for a given header name with knows which type they should have
///
/// This iterator will automatically try to cast each header body of this
/// header to `H::Component`, i.e. the type this body _should_ have.
pub struct TypedBodiesMut<'a, H>
where
    H: HeaderKind,
{
    inner: UntypedBodiesMut<'a>,
    _marker: PhantomData<H>,
}

impl<'a, H> From<UntypedBodiesMut<'a>> for TypedBodiesMut<'a, H>
where
    H: HeaderKind,
{
    fn from(untyped: UntypedBodiesMut<'a>) -> Self {
        Self::new(untyped)
    }
}

impl<'a, H> TypedBodiesMut<'a, H>
where
    H: HeaderKind,
{
    fn new(inner: UntypedBodiesMut<'a>) -> Self {
        TypedBodiesMut {
            inner,
            _marker: PhantomData,
        }
    }
}

impl<'a, H> Iterator for TypedBodiesMut<'a, H>
where
    H: HeaderKind,
{
    type Item = Result<&'a mut Header<H>, HeaderTypeError>;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|tobj| {
            tobj.downcast_mut::<H>()
                .ok_or_else(|| HeaderTypeError::new(H::name()))
        })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl<'a, H> ExactSizeIterator for TypedBodiesMut<'a, H>
where
    H: HeaderKind,
{
    fn len(&self) -> usize {
        self.inner.len()
    }
}

impl<'a, H> Debug for TypedBodiesMut<'a, H>
where
    H: HeaderKind,
{
    fn fmt(&self, fter: &mut fmt::Formatter) -> fmt::Result {
        fter.write_str("TypedBodiesMut { .. }")
    }
}

/// Create a header map from a list of header's with ther fields
///
/// # Example
///
/// ```
/// # #[macro_use]
/// # extern crate mail_headers;
/// # use mail_headers::headers::*;
/// # use mail_headers::error::ComponentCreationError;
/// # fn main() { (|| -> Result<(), ComponentCreationError> {
/// let map = headers! {
///     _From: ["bobo@nana.test"],
///     Subject: "hy there"
/// }?;
/// # Ok(()) })(); }
/// ```
#[macro_export]
macro_rules! headers {
    ($($header:ty : $val:expr),*) => ({
        //FIXME[rust/catch block] use catch block once available
        (|| -> ::std::result::Result<$crate::HeaderMap, $crate::error::ComponentCreationError> {
            let mut map = $crate::HeaderMap::new();
            $(
                map.insert(<$header as $crate::HeaderKind>::auto_body($val)?);
            )*
            Ok(map)
        })()
    });
}

/// HeaderMapValidator is just a function pointer,
/// but it does not implement Hash so we wrap it
/// and implement Hash on it. Note that some function
/// pointers implement Hash/Eq and other doesn't,
/// which is caused by some limitations with wildcard
/// implementations
#[derive(Copy, Clone)]
struct ValidatorHashWrapper(HeaderMapValidator);

impl ValidatorHashWrapper {
    fn identity_repr(&self) -> usize {
        self.0 as usize
    }
}

impl PartialEq<Self> for ValidatorHashWrapper {
    fn eq(&self, other: &Self) -> bool {
        self.identity_repr() == other.identity_repr()
    }
}

impl Eq for ValidatorHashWrapper {}

impl Debug for ValidatorHashWrapper {
    fn fmt(&self, fter: &mut fmt::Formatter) -> fmt::Result {
        write!(fter, "ValidatorHashWrapper(0x{:x})", self.identity_repr())
    }
}

impl Hash for ValidatorHashWrapper {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_usize(self.identity_repr())
    }
}

pub fn check_header_count_max_one(
    name: HeaderName,
    map: &HeaderMap,
) -> Result<(), HeaderValidationError> {
    let valid = map.get_untyped(name).len() <= 1;
    if valid {
        Ok(())
    } else {
        Err(HeaderValidationError::from(
            BuildInValidationError::MoreThenOne {
                header_name: name.as_str(),
            },
        ))
    }
}

#[cfg(test)]
mod test {
    use failure::Context;
    use soft_ascii_string::SoftAsciiStr;

    use internals::encoder::{EncodableInHeader, EncodingWriter};
    use internals::error::{EncodingError, EncodingErrorKind};

    use error::{ComponentCreationError, HeaderValidationError};
    use header_components::RawUnstructured;
    use HeaderTryFrom;

    use super::*;

    use self::bad_headers::{Comments as BadComments, Subject as BadSubject};
    use self::bad_headers2::Comments2 as BadComments2;
    use self::good_headers::*;

    #[derive(Debug, Clone, Eq, PartialEq, Hash)]
    pub struct OtherComponent;

    impl HeaderTryFrom<()> for OtherComponent {
        fn try_from(_: ()) -> Result<OtherComponent, ComponentCreationError> {
            Ok(OtherComponent)
        }
    }
    impl EncodableInHeader for OtherComponent {
        fn encode(&self, _encoder: &mut EncodingWriter) -> Result<(), EncodingError> {
            Err(EncodingError::from(EncodingErrorKind::Other {
                kind: "encoding is not implemented",
            }))
        }

        fn boxed_clone(&self) -> Box<EncodableInHeader> {
            Box::new(self.clone())
        }
    }

    mod good_headers {
        use header_components;
        def_headers! {
            test_name: validate_header_names,
            scope: header_components,
            Subject, unchecked { "Subject" }, RawUnstructured, maxOne, None,
            Comments, unchecked { "Comments" }, RawUnstructured, multi, None
        }
    }

    mod bad_headers {
        def_headers! {
            test_name: validate_header_names,
            scope: super,
            Subject, unchecked { "Subject" },  OtherComponent, maxOne, None,
            Comments, unchecked { "Comments" }, OtherComponent, multi, None
        }
    }
    mod bad_headers2 {
        def_headers! {
            test_name: validate_header_names2,
            scope: super,
            Comments2, unchecked { "Comments" }, OtherComponent, maxOne, None
        }
    }

    const TEXT_1: &str = "Random stuff XD";
    const TEXT_2: &str = "Having a log of fun, yes a log!";

    test!(max_one_mixup {
        let headers = headers! {
            BadComments2: (),
            BadComments: ()
        }?;

        let res = headers.use_contextual_validators();
        if let Err(HeaderValidationError::BuildIn(berr)) = res {
            if let BuildInValidationError::MaxOneInconsistency { ..} = berr.get_context() {
                return Ok(());
            }
            panic!("unexpected error: {:?}", berr);
        }
        panic!("unexpected result: {:?}", res);
    });

    #[test]
    fn headers_macro() {
        let headers = headers! {
            Comments: TEXT_1,
            Subject: TEXT_2
        }
        .unwrap();

        let count = headers
            // all headers _could_ have multiple values, through neither
            // ContentType nor Subject do have multiple value
            .get(Comments)
            .map(|h: Result<&Header<Comments>, HeaderTypeError>| {
                let v = h.expect("the trait object to be downcastable to Header<Comments>");
                assert_eq!(v.as_str(), TEXT_1);
            })
            .count();
        assert_eq!(1, count);

        let count = headers
            .get(Subject)
            .map(|h: Result<&Header<Subject>, HeaderTypeError>| {
                let val = h.expect("the trait object to be downcastable to Header<Subject>");
                assert_eq!(val.as_str(), TEXT_2);
            })
            .count();
        assert_eq!(1, count);
    }

    #[test]
    fn get_single() {
        let headers = headers! {
            Subject: "abc"
        }
        .unwrap();

        assert_eq!(
            "abc",
            headers
                .get_single(Subject)
                .unwrap() //Some
                .unwrap() //Result
                .as_str()
        );
    }

    #[test]
    fn get_single_cast_error() {
        let headers = headers! {
            Subject: "abc"
        }
        .unwrap();

        let res = headers.get_single(BadSubject);
        assert_err!(res.expect("where did the header go?"));
    }

    #[test]
    fn get() {
        let headers = headers! {
            Subject: "abc",
            Comments: "1st",
            BadComments: ()
        }
        .unwrap();

        let mut res = headers.get(Comments);

        assert_eq!(res.size_hint(), (2, Some(2)));

        assert_eq!("1st", assert_ok!(res.next().unwrap()).as_str());

        assert_err!(res.next().unwrap());

        assert!(res.next().is_none())
    }

    #[test]
    fn get_untyped() {
        let headers = headers! {
            Subject: "abc",
            Comments: "1st",
            BadComments: ()
        }
        .unwrap();

        let res = headers
            .get_untyped(Subject::name())
            .map(|entry| entry.downcast_ref::<Subject>().unwrap().as_str())
            .collect::<Vec<_>>();

        assert_eq!(res.as_slice(), &["abc"]);

        let mut res = headers.get_untyped(Comments::name());

        assert_eq!((2, Some(2)), res.size_hint());

        assert_eq!(
            res.next()
                .unwrap()
                .downcast_ref::<Comments>()
                .unwrap()
                .as_str(),
            "1st"
        );

        assert_eq!((1, Some(1)), res.size_hint());

        assert_eq!(
            res.next()
                .unwrap()
                .downcast_ref::<BadComments>()
                .unwrap()
                .body(),
            &OtherComponent
        );

        assert!(res.next().is_none());
    }

    #[test]
    fn fmt_debug() {
        let headers = headers! {
            Subject: "hy there"
        }
        .unwrap();

        let res = format!("{:?}", headers);
        assert_eq!(
            "HeaderMap { Subject: RawUnstructured { text: Input(Owned(\"hy there\")) }, }",
            res.as_str()
        );
    }

    test!(combine_keeps_order {
        let mut headers = headers! {
            XComment: "ab@c"
        }?;

        headers.insert_all(headers! {
            Subject: "hy there",
            Comments: "magic+spell"
        }?);

        assert_eq!(
            &[
                "X-Comment",
                "Subject",
                "Comments"
            ],
            headers.into_iter()
                .map(|(name, _val)| name.as_str())
                .collect::<Vec<_>>()
                .as_slice()
        );
    });

    test!(remove_1 {
        let mut headers = headers!{
            Comments: "a",
            Subject: "b",
            Comments: "c",
            Comments: "d"
        }?;

        assert_eq!( false, headers.remove(XComment::name()));
        assert_eq!( true, headers.remove(Subject::name()));

        assert_eq!( 3, headers.iter().count() );

        let values = headers.get(Comments)
            .map(|comp| comp.unwrap().as_str() )
            .collect::<Vec<_>>();

        assert_eq!(
            &[ "a", "c", "d" ],
            values.as_slice()
        );
    });

    test!(remove_2 {
        let mut headers = headers!{
            Comments: "a",
            Subject: "b",
            Comments: "c",
            Comments: "d"
        }?;

        assert_eq!(true, headers.remove(Comments::name()));
        assert_eq!(false, headers.remove(Comments::name()));

        assert_eq!(1, headers.iter().count());

        let values = headers.get(Subject)
            .map(|comp| comp.unwrap().as_str())
            .collect::<Vec<_>>();

        assert_eq!(
            &[ "b" ],
            values.as_slice()
        );
    });

    #[derive(Default, Copy, Clone)]
    struct XComment;
    impl HeaderKind for XComment {
        type Component = RawUnstructured;

        fn name() -> HeaderName {
            HeaderName::new(SoftAsciiStr::from_unchecked("X-Comment")).unwrap()
        }

        const VALIDATOR: Option<fn(&HeaderMap) -> Result<(), HeaderValidationError>> =
            Some(__validator);

        const MAX_ONE: bool = false;
    }

    //some stupid but simple validator
    fn __validator(map: &HeaderMap) -> Result<(), HeaderValidationError> {
        if map.get_untyped(Comments::name()).len() != 0 {
            return Err(HeaderValidationError::Custom(
                Context::new("can't have X-Comment and Comments in same mail").into(),
            ));
        }
        Ok(())
    }

    test!(contains_works {
        let map = headers! {
            Subject: "soso"
        }?;

        assert_eq!( true,  map.contains( Subject::name()  ));
        assert_eq!( true,  map.contains( Subject          ));
        assert_eq!( false, map.contains( Comments::name() ));
        assert_eq!( false, map.contains( Comments         ));
    });

    test!(use_validator_ok {
        let map = headers! {
            XComment: "yay",
            Subject: "soso"
        }?;

        assert_ok!(map.use_contextual_validators());
    });

    test!(use_validator_err {
        let map = headers! {
            XComment: "yay",
            Comments: "oh no",
            Subject: "soso"
        }?;

        assert_err!(map.use_contextual_validators());
    });

    test!(has_len {
        let map = headers! {
            XComment: "yay",
            Comments: "oh no",
            Subject: "soso"
        }?;

        assert_eq!(3, map.len());
    });

    test!(does_not_conflic_with_custom_result_type {
        #[allow(unused)]
        type Result<T> = ::std::result::Result<T, ()>;

        let map = headers! {
            Subject: "yay"
        }?;

        assert_eq!(1, map.len());
    });
}
