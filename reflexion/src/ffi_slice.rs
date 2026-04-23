use std::slice;

pub trait Ref {
    type Inner;
}

impl<T> Ref for &T {
    type Inner = T;
}

impl<T> Ref for &mut T {
    type Inner = T;
}

/// Utility to pass slice across FII bounds, easily convertible into rust slices
#[repr(C)]
pub struct FiiSlice<T: Ref> {
    data: *const T::Inner,
    len: usize,
}

impl<'a, T> From<&'a [T]> for FiiSlice<&'a T> {
    fn from(value: &[T]) -> Self {
        Self {
            data: value.as_ptr(),
            len: value.len(),
        }
    }
}

impl<'a, T> From<FiiSlice<&'a T>> for &'a [T] {
    fn from(value: FiiSlice<&T>) -> Self {
        unsafe { slice::from_raw_parts(value.data, value.len) }
    }
}

impl<'a, T> From<&'a mut [T]> for FiiSlice<&'a mut T> {
    fn from(value: &mut [T]) -> Self {
        Self {
            data: value.as_ptr(),
            len: value.len(),
        }
    }
}

impl<'a, T> From<FiiSlice<&'a mut T>> for &'a mut [T] {
    fn from(value: FiiSlice<&mut T>) -> Self {
        unsafe { slice::from_raw_parts_mut(value.data as *mut T, value.len) }
    }
}