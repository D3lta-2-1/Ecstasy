use crate::erased::ErasedMutPointer;
use std::marker::PhantomData;
use std::mem;

/// A place where a thing is about to be dropped. If nothing is done, the underlying value is dropped.
#[repr(C)]
pub struct DropLocation<'a> {
    pub(crate) location: ErasedMutPointer,
    _phantom: PhantomData<&'a mut ()>,
}

impl<'a> DropLocation<'a> {
    pub unsafe fn at(location: ErasedMutPointer) -> Self {
        Self {
            location,
            _phantom: PhantomData,
        }
    }

    /// the passed value should be mem::forget just after
    pub unsafe fn at_hard<T>(location: &mut T) -> Self {
        unsafe {
            Self {
                location: ErasedMutPointer::from_mut(location),
                _phantom: PhantomData,
            }
        }
    }

    /// init this location from a "concret" value, panic if the TypeInfo don't match required type
    pub fn read<T>(self) -> T {
        unsafe {
            let value = self.location.read::<T>();
            mem::forget(self);
            value
        }
    }
}

impl<'a> Drop for DropLocation<'a> {
    /// this might trigger a double panic, but need to be stored...
    fn drop(&mut self) {
        unsafe {
            self.location.drop_in_place();
        }
    }
}
