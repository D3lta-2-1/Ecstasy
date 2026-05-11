use std::mem::MaybeUninit;

use crate::ffi_slice::FfiSlice;

/// An ABI safe iterator for Arrays
#[repr(C)]
pub struct FfiCollectionIter<'a, T> {
    slice: FfiSlice<&'a mut MaybeUninit<T>>,
    i: usize,
}

impl<T> FfiCollectionIter<'_, T> {
    pub fn from_array<const N: usize, RETURN>(
        array: [T; N],
        scope: impl FnOnce(FfiCollectionIter<'_, T>) -> RETURN,
    ) -> RETURN {
        let mut inner: [MaybeUninit<T>; N] = MaybeUninit::new(array).into();
        let slice = inner.as_mut_slice().into();
        let iter = FfiCollectionIter { slice, i: 0 };
        scope(iter)
    }
}

impl<'a, T> Iterator for FfiCollectionIter<'a, T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        let value = self.slice.get(self.i)?;
        self.i += 1;
        unsafe { Some(value.assume_init_read()) }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.slice.len(), None)
    }
}

impl<'a, T> ExactSizeIterator for FfiCollectionIter<'a, T> {}

impl<'a, T> Drop for FfiCollectionIter<'a, T> {
    fn drop(&mut self) {
        while self.next().is_some() {} // drop the remaining values
    }
}
