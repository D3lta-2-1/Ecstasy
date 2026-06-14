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

/// sort on array, and applies the same perumtation to the other one
pub fn sort<T: Ord, U>(keys: &mut [T], values: &mut [U]) {
    assert_eq!(keys.len(), values.len());
    for i in 1..keys.len() {
        for j in (0..i).rev() {
            if keys[j + 1] < keys[j] {
                keys.swap(j + 1, j);
                values.swap(j + 1, j);
            } else {
                break;
            }
        }
    }
}

#[test]
fn test_sort() {
    let mut keys = [5, 4, 3, 2, 1];
    let mut values = [1, 2, 3, 4, 5];
    sort(&mut keys, &mut values);
    assert_eq!(keys, [1, 2, 3, 4, 5]);
    assert_eq!(values, [5, 4, 3, 2, 1]);
}
