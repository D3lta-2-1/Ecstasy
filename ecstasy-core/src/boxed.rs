use std::ops::{Deref, DerefMut};

use reflexion::{
    drop_location::DropLocation,
    erased::{Any, ErasedMutPointer, ZeroSized},
};

/// An container to store one "unknown" type.
pub struct Box<PTR: ZeroSized = Any> {
    ptr: ErasedMutPointer<PTR>,
}

impl<PTR: ZeroSized> Box<PTR> {
    pub fn new(drop_location: DropLocation<PTR>) -> Self {
        unsafe {
            let mut ptr = ErasedMutPointer::<PTR>::dangling(drop_location.type_info());
            ptr.allocate(1);
            ptr.write_drop_location(drop_location);
            Self { ptr }
        }
    }
}

impl<PTR: ZeroSized> Deref for Box<PTR> {
    type Target = PTR;

    fn deref(&self) -> &Self::Target {
        unsafe { self.ptr.as_zeros_sized_ref() }
    }
}

impl<PTR: ZeroSized> DerefMut for Box<PTR> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { self.ptr.as_zeros_sized_mut() }
    }
}

impl<PTR: ZeroSized> Drop for Box<PTR> {
    fn drop(&mut self) {
        unsafe {
            self.ptr.drop_in_place();
        }
    }
}
