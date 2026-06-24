use crate::{
    erased::{Any, ErasedMutPointer, ZeroSized},
    typeinfo::TypeInfo,
};
use std::{marker::PhantomData, mem};

/// A place where a thing is about to be dropped. If nothing is done, the underlying value is dropped.
#[repr(C)]
pub struct DropLocation<'a, PTR: ZeroSized = Any> {
    pub(crate) location: ErasedMutPointer<PTR>,
    _phantom: PhantomData<&'a mut ()>,
}

impl<'a, PTR: ZeroSized> DropLocation<'a, PTR> {
    pub unsafe fn at(location: ErasedMutPointer<PTR>) -> Self {
        Self {
            location,
            _phantom: PhantomData,
        }
    }

    /// the passed value should be mem::forget after this call, the end of the borrow mean that the resource is already released
    pub unsafe fn at_hard<T>(location: &mut T) -> Self {
        unsafe {
            Self {
                location: ErasedMutPointer::from_mut(location),
                _phantom: PhantomData,
            }
        }
    }

    pub fn type_info(&self) -> TypeInfo {
        self.location.type_info
    }

    /// init this location from a "concrete" value, panic if the TypeInfo don't match required type
    pub fn read<T: Sized>(self) -> T {
        unsafe {
            let value = self.location.read::<T>();
            mem::forget(self);
            value
        }
    }
}

impl<'a, PTR: ZeroSized> Drop for DropLocation<'a, PTR> {
    /// this might trigger a double panic, but need to be stored...
    fn drop(&mut self) {
        unsafe {
            self.location.drop_in_place();
        }
    }
}
