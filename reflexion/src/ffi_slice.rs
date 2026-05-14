use std::{
    hash::Hash,
    ops::{Deref, DerefMut},
    ptr::NonNull,
    slice,
};

pub trait Ref {
    type Inner: ?Sized;
}

impl<T> Ref for &T {
    type Inner = T;
}

impl<T> Ref for &mut T {
    type Inner = T;
}

/// Utility to pass slice across FII bounds, easily convertible into rust slices
/// ``Option<FiiSlice<T>>`` is guarantee to be the same size as ``FiiSlice<T>`` thanks to the null pointer optimization
#[repr(C)]
pub struct FfiSlice<T: Ref> {
    data: NonNull<T::Inner>,
    len: usize,
}

impl<'a, T> FfiSlice<&T> {
    pub const fn from_slice(slice: &[T]) -> Self {
        Self {
            data: NonNull::new(slice.as_ptr() as *mut T).unwrap(),
            len: slice.len(),
        }
    }
}

impl<'a> FfiSlice<&u8> {
    pub const fn from_str(slice: &str) -> Self {
        Self::from_slice(slice.as_bytes())
    }
}

impl<'a, T> FfiSlice<&mut T> {
    pub const fn from_slice(slice: &mut [T]) -> Self {
        Self {
            data: NonNull::new(slice.as_ptr() as *mut T).unwrap(),
            len: slice.len(),
        }
    }
}

impl<'a, T> Clone for FfiSlice<&'a T> {
    fn clone(&self) -> Self {
        Self {
            data: self.data.clone(),
            len: self.len.clone(),
        }
    }
}

impl<'a, T> Copy for FfiSlice<&'a T> {}

impl<'a, T: PartialEq> PartialEq for FfiSlice<&'a T> {
    fn eq(&self, other: &Self) -> bool {
        let a: &'a [T] = self.clone().into();
        let b: &'a [T] = other.clone().into();
        a == b
    }
}

impl<'a, T: Eq> Eq for FfiSlice<&'a T> {}

impl<'a, T: Hash> Hash for FfiSlice<&'a T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        let slice: &'a [T] = self.clone().into();
        slice.hash(state);
    }
}

impl<'a, T> From<&'a [T]> for FfiSlice<&'a T> {
    fn from(slice: &[T]) -> Self {
        Self::from_slice(slice)
    }
}

impl<'a, T> From<FfiSlice<&'a T>> for &'a [T] {
    fn from(slice: FfiSlice<&T>) -> Self {
        unsafe { slice::from_raw_parts(slice.data.as_ptr(), slice.len) }
    }
}

impl<'a, T> From<&'a mut [T]> for FfiSlice<&'a mut T> {
    fn from(slice: &mut [T]) -> Self {
        Self::from_slice(slice)
    }
}

impl<'a, T> From<FfiSlice<&'a mut T>> for &'a mut [T] {
    fn from(slice: FfiSlice<&mut T>) -> Self {
        unsafe { slice::from_raw_parts_mut(slice.data.as_ptr(), slice.len) }
    }
}

impl<'a, T> AsRef<[T]> for FfiSlice<&'a T> {
    fn as_ref(&self) -> &[T] {
        unsafe { slice::from_raw_parts(self.data.as_ptr(), self.len) }
    }
}

impl<'a, T> AsMut<[T]> for FfiSlice<&'a mut T> {
    fn as_mut(&mut self) -> &mut [T] {
        unsafe { slice::from_raw_parts_mut(self.data.as_ptr(), self.len) }
    }
}

impl<'a, T> Deref for FfiSlice<&'a T> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        unsafe { slice::from_raw_parts(self.data.as_ptr(), self.len) }
    }
}

impl<'a, T> Deref for FfiSlice<&'a mut T> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        unsafe { slice::from_raw_parts(self.data.as_ptr(), self.len) }
    }
}

impl<'a, T> DerefMut for FfiSlice<&'a mut T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { slice::from_raw_parts_mut(self.data.as_ptr(), self.len) }
    }
}

#[test]
fn ensure_layout() {
    use std::alloc::Layout;
    assert_eq!(
        Layout::new::<FfiSlice<&i32>>(),
        Layout::new::<Option<FfiSlice<&i32>>>()
    )
}
