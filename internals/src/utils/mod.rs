//! Some more general utilities.
//!
//! Or with other words, thinks which
//! (currently) have no other place to
//! be placed in.
use std::any::TypeId;
use std::cell::RefCell;
use std::fmt::{self, Debug};
use std::mem;

/// Helper for implementing debug for an iterable think where the think on itself is irrelevant.
pub struct DebugIterableOpaque<I> {
    one_use_inner: RefCell<I>,
}

impl<I> DebugIterableOpaque<I> {
    pub fn new(one_use_inner: I) -> Self {
        let one_use_inner = RefCell::new(one_use_inner);
        DebugIterableOpaque { one_use_inner }
    }
}
impl<I> Debug for DebugIterableOpaque<I>
where
    I: Iterator,
    I::Item: Debug,
{
    fn fmt(&self, fter: &mut fmt::Formatter) -> fmt::Result {
        let mut borrow = self.one_use_inner.borrow_mut();
        fter.debug_list().entries(&mut *borrow).finish()
    }
}

//FIXME[rust/fat pointer cast]: make it ?Sized once it's supported by rust
///
/// Used to undo type erasure in a generic context,
/// roughly semantically eqivalent to creating a `&Any`
/// type object from the input and then using `downcast_ref::<EXP>()`,
/// except that it does not require the cration of a
/// trait object as a step inbetween.
///
/// Note:
/// This function can be used for some form of specialisation,
/// (not just in a performence sense) but all "specialization path"
/// have to be known when writing the unspeciallized version and
/// it is easy to make functions behave in a unexpected (but safe)
/// way so use with care.
///
///
#[inline(always)]
pub fn uneraser_ref<GOT: 'static, EXP: 'static>(inp: &GOT) -> Option<&EXP> {
    if TypeId::of::<GOT>() == TypeId::of::<EXP>() {
        //SAFE: the GOT type is exact the same as the EXP type,
        // the compiler just does not know this due to type erasure wrt.
        // generic types
        let res: &EXP = unsafe { mem::transmute::<&GOT, &EXP>(inp) };
        Some(res)
    } else {
        None
    }
}

//FIXME[rust/fat pointer cast]: make it ?Sized once it's supported by rust
#[doc(hidden)]
#[inline(always)]
pub fn uneraser_mut<GOT: 'static, EXP: 'static>(inp: &mut GOT) -> Option<&mut EXP> {
    if TypeId::of::<GOT>() == TypeId::of::<EXP>() {
        //SAFE: the GOT type is exact the same as the EXP type,
        // the compiler just does not know this due to type erasure wrt.
        // generic types
        let res: &mut EXP = unsafe { mem::transmute::<&mut GOT, &mut EXP>(inp) };
        Some(res)
    } else {
        None
    }
}

//FIXME: only works if the rust compiler get's a bit more clever or a bit less (either is fine)
//#[inline(always)]
//pub fn uneraser<GOT: 'static, EXP: 'static>( inp: GOT ) -> Result<EXP, GOT> {
//    if TypeId::of::<GOT>() == TypeId::of::<EXP>() {
//        //SAFE: the GOT type is exact the same as the EXP type,
//        // the compiler just does not know this due to type erasure wrt.
//        // generic types
//        Ok( unsafe { mem::transmute::<GOT, EXP>( inp ) } )
//    } else {
//        Err( inp )
//    }
//}

//fn get_flat_byte_repr<T>(val: &T) -> Vec<u8> {
//    let count = mem::size_of::<T>();
//    let mut out = Vec::with_capacity(count);
//    let byte_ptr = val as *const T as *const u8;
//    for offset in 0..count {
//        out.push( unsafe {
//            *byte_ptr.offset(offset as isize)
//        })
//    }
//    out
//}

/// returns true if this a not first byte from a multi byte utf-8
///
/// This will return false:
/// - on all us-ascii  chars (as u8)
/// - on the first byte of a multi-byte utf-8 char
///
pub fn is_utf8_continuation_byte(b: u8) -> bool {
    // all additional bytes (and only them) in utf8 start with 0b10xxxxxx so while
    (b & 0b11000000) == 0b10000000
}

/// Faster insertion of byte slices into a byte vector.
pub fn vec_insert_bytes(target: &mut Vec<u8>, idx: usize, source: &[u8]) {
    use std::ptr::copy;

    if idx > target.len() {
        panic!(
            "index out of bounds: the len is {} but the index is {}",
            target.len(),
            idx
        );
    }

    let old_len = target.len();
    let insertion_len = source.len();
    let source_ptr = source.as_ptr();
    let insertion_point = unsafe {
        // SAFE: we panic if idx > target.len(), through idx == target.len() is fine
        target.as_mut_ptr().offset(idx as isize)
    };
    let moved_data_len = old_len - idx;

    target.reserve(insertion_len);

    unsafe {
        // SAFE 1: we reserved insertion_len and insertion_point is at most old_len
        //         so offset is fine
        // SAFE 2: insertion_point + insertion_len + moved_data_len needs to be
        //         <= target + target.capacity(). By replacing variables:
        //         - insertion_point + insertion_len + moved_data_len <= target + capacity
        //         - target + idx + insertion_len + old_len - idx <= target + capacity
        //         - target + idx + insertion_len + old_len - idx <= target + old_len + insertion_len
        //         - idx + insertion_len + old_len - idx <= old_len + insertion_len
        //         - idx - idx <= 0
        //         - 0 <= 0  [Q.E.D]
        copy(
            /*src*/ insertion_point,
            /*dest*/ insertion_point.offset(insertion_len as isize),
            /*count*/ moved_data_len,
        );

        // SAFE: insertion_point + insertion_len needs to be <= target.capacity()
        //   which is guaranteed as we reserve insertion len and insertion_point is
        //   at most old len.
        copy(source_ptr, insertion_point, insertion_len);

        // SAFE: we reserved insertion_len bytes
        target.set_len(old_len + insertion_len)
    }
}

#[cfg(test)]
mod tests {
    use super::vec_insert_bytes;

    #[test]
    fn inserting_slices_at_beginning() {
        let mut base = vec![0u8, 1u8, 2u8, 3u8];
        let new = &[10u8, 11];

        vec_insert_bytes(&mut base, 0, new);

        assert_eq!(&*base, &[10u8, 11, 0, 1, 2, 3]);
        assert!(base.capacity() >= 6);
    }

    #[test]
    fn inserting_slices_at_end() {
        let mut base = vec![0u8, 1u8, 2u8, 3u8];
        let new = &[10u8, 11];

        let end = base.len();
        vec_insert_bytes(&mut base, end, new);

        assert_eq!(&*base, &[0u8, 1, 2, 3, 10, 11]);
        assert!(base.capacity() >= 6);
    }

    #[test]
    fn inserting_slices_in_the_middle() {
        let mut base = vec![0u8, 1u8, 2u8, 3u8];
        let new = &[10u8, 11];

        vec_insert_bytes(&mut base, 1, new);

        assert_eq!(&*base, &[0u8, 10, 11, 1, 2, 3]);
        assert!(base.capacity() >= 6);
    }

    #[test]
    fn inserting_slices_large_in_the_middle() {
        let mut base = vec![0u8, 1u8, 2u8, 3u8];
        let new = &[10u8, 11, 12, 13, 14, 15, 16];

        vec_insert_bytes(&mut base, 1, new);

        assert_eq!(&*base, &[0u8, 10, 11, 12, 13, 14, 15, 16, 1, 2, 3]);
        assert!(base.capacity() >= 11);
    }

    #[should_panic]
    #[test]
    fn insert_out_of_bound() {
        let mut base = vec![0u8, 1u8, 2u8, 3u8];
        let new = &[10u8];

        vec_insert_bytes(&mut base, 10, new);
    }
}
