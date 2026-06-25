use std::{array, ops::Index};

/// Utility trait to use arrays in generics
pub trait Array<T>:
    AsRef<[T]> + AsMut<[T]> + IntoIterator<Item = T> + Index<usize, Output = T> + Sized
{
    const LEN: usize;
    fn from_fn(f: impl FnMut(usize) -> T) -> Self;
    fn for_each(&mut self, f: impl FnMut(&mut T));
    fn map<ARRAY, U>(self, f: impl FnMut(T) -> U) -> ARRAY
    where
        ARRAY: Array<U>,
        T: Copy;
}

impl<T, const LEN: usize> Array<T> for [T; LEN] {
    const LEN: usize = LEN;
    fn from_fn(f: impl FnMut(usize) -> T) -> Self {
        array::from_fn(f)
    }

    fn for_each(&mut self, f: impl FnMut(&mut T)) {
        self.iter_mut().for_each(f)
    }

    fn map<ARRAY, U>(self, mut f: impl FnMut(T) -> U) -> ARRAY
    where
        ARRAY: Array<U>,
        T: Copy,
    {
        ARRAY::from_fn(|i| f(self[i]))
    }
}
