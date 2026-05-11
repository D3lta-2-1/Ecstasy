use std::{
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

impl<'a, T> From<&'a [T]> for FfiSlice<&'a T> {
    fn from(value: &[T]) -> Self {
        Self {
            data: NonNull::new(value.as_ptr() as *mut T).unwrap(),
            len: value.len(),
        }
    }
}

impl<'a, T> From<FfiSlice<&'a T>> for &'a [T] {
    fn from(value: FfiSlice<&T>) -> Self {
        unsafe { slice::from_raw_parts(value.data.as_ptr(), value.len) }
    }
}

impl<'a, T> From<&'a mut [T]> for FfiSlice<&'a mut T> {
    fn from(value: &mut [T]) -> Self {
        Self {
            data: NonNull::new(value.as_mut_ptr()).unwrap(),
            len: value.len(),
        }
    }
}

impl<'a, T> From<FfiSlice<&'a mut T>> for &'a mut [T] {
    fn from(value: FfiSlice<&mut T>) -> Self {
        unsafe { slice::from_raw_parts_mut(value.data.as_ptr(), value.len) }
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
