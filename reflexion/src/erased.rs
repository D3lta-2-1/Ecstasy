//! Provide a bunch on erased handle following rust goods manners
//! Most of the building blocks of this module are equivalents of mutable pointers, references and mutables references
//! when the value isn't known at compile time

use crate::{
    drop_location::DropLocation,
    typeinfo::{TypeInfo, TypeInfoProvider},
};
use std::{marker::PhantomData, mem, ptr::NonNull};

/// A pointer encapsulation without any type information.
/// This is used to store pointers to any type in a generic way.
/// It can be viewed as a wide pointer that carries a reference to the type information of the pointed type.

/// This is still a fairly low level abstraction, this doesn't really care if it content have been initialized
/// or not, therefor, it doesn't perform any kind of ownership
/// It is equivalent of a NonNull ptr type.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ErasedMutPointer {
    pub data: NonNull<u8>,
    pub type_info: TypeInfo,
}

impl ErasedMutPointer {
    /// return a pointer set to null, without any type associated
    /// any operation on the pointer will fail
    pub fn empty() -> Self {
        Self::dangling(None)
    }

    pub fn dangling(type_info: TypeInfo) -> Self {
        Self {
            type_info,
            data: NonNull::dangling(),
        }
    }

    pub unsafe fn from_mut<T: Sized>(data: &mut T) -> Self {
        unsafe {
            Self {
                type_info: T::TYPE_INFO,
                data: NonNull::new_unchecked(data as *mut T as *mut u8),
            }
        }
    }

    pub fn align(self) -> usize {
        let Some(type_info) = self.type_info else {
            panic!("type info doesn't exist")
        };
        type_info.layout.align
    }

    pub fn size(self) -> usize {
        let Some(type_info) = self.type_info else {
            panic!("type info doesn't exist")
        };
        type_info.layout.size
    }

    /// allocate a memory block
    pub unsafe fn allocate(&mut self, count: usize) {
        let Some(type_info) = self.type_info else {
            panic!("type info doesn't exist")
        };
        self.data = if type_info.layout.size == 0 {
            NonNull::dangling()
        } else {
            unsafe {
                NonNull::new(std::alloc::alloc(
                    std::alloc::Layout::from_size_align_unchecked(
                        type_info.layout.size * count,
                        type_info.layout.align,
                    ),
                ))
                .expect("allocation failed")
            }
        };
    }

    /// reallocate a memory block
    pub unsafe fn reallocate(&mut self, new_count: usize) {
        let Some(type_info) = self.type_info else {
            panic!("type info doesn't exist")
        };
        self.data = if type_info.layout.size == 0 {
            NonNull::dangling()
        } else {
            unsafe {
                NonNull::new(std::alloc::realloc(
                    self.data.as_ptr(),
                    std::alloc::Layout::from_size_align_unchecked(
                        type_info.layout.size,
                        type_info.layout.align,
                    ),
                    type_info.layout.size * new_count,
                ))
                .expect("allocation failed")
            }
        };
    }

    /// free the associated memory block.
    pub unsafe fn deallocate(self, count: usize) {
        let Some(type_info) = self.type_info else {
            panic!("type info doesn't exist")
        };
        if type_info.layout.size == 0 {
            return; // no need to deallocate zero-sized types
        }
        unsafe {
            std::alloc::dealloc(
                self.data.as_ptr(),
                std::alloc::Layout::from_size_align_unchecked(
                    type_info.layout.size * count,
                    type_info.layout.align,
                ),
            );
        }
    }

    /// offset the pointer using the stored type size.
    pub unsafe fn offset(self, offset: usize) -> Self {
        let Some(type_info) = self.type_info else {
            panic!("type info doesn't exist")
        };
        unsafe {
            ErasedMutPointer {
                type_info: self.type_info,
                data: self.data.offset((offset * type_info.layout.size) as isize),
            }
        }
    }

    pub unsafe fn copy_nonoverlapping_from(&self, source: ErasedMutPointer) {
        let Some(type_info) = self.type_info else {
            panic!("type info doesn't exist")
        };
        assert_eq!(
            self.type_info, source.type_info,
            "Type mismatch: cannot copy data of type {:?} to location of type {:?}",
            source.type_info, self.type_info
        );
        unsafe {
            std::ptr::copy_nonoverlapping(
                source.data.as_ptr(),
                self.data.as_ptr(),
                type_info.layout.size,
            );
        }
    }

    pub unsafe fn drop_in_place(self) {
        let Some(type_info) = self.type_info else {
            panic!("type info doesn't exist")
        };
        unsafe {
            (type_info.destructor)(self.data.as_ptr());
        }
    }

    pub unsafe fn read<T>(self) -> T {
        unsafe { (self.data.as_ptr() as *const T).read() }
    }

    pub unsafe fn write<T>(self, src: T) {
        unsafe {
            assert_eq!(
                self.type_info,
                T::TYPE_INFO,
                "Type mismatch: expected {:?}, found {:?}",
                self.type_info,
                T::TYPE_INFO
            );
            std::ptr::write(self.data.as_ptr() as *mut T, src)
        }
    }

    pub unsafe fn write_drop_location(self, location: DropLocation) {
        unsafe { self.copy_nonoverlapping_from(location.location) }
        mem::forget(location);
    }

    /// build a reference, the lifetime should be provided by the caller
    pub unsafe fn as_erased_ref<'a>(self) -> ErasedRef<'a> {
        ErasedRef {
            ptr: self,
            _phantom: PhantomData,
        }
    }

    /// build a reference, the lifetime should be provided by the caller
    pub unsafe fn as_erased_mut<'a>(self) -> ErasedMut<'a> {
        ErasedMut {
            ptr: self,
            _phantom: PhantomData,
        }
    }
}

/// a reference for ErasedDataType
#[repr(transparent)]
#[derive(Debug, Clone, Copy)]
pub struct ErasedRef<'a> {
    ptr: ErasedMutPointer,
    _phantom: PhantomData<&'a ()>,
}

impl<'a> ErasedRef<'a> {
    /// Safety : this function will compare the layouts of the objects and panic if they don't match
    /// It's up to the user to cast back to the right type
    /// However, this abstraction assume that the pointed value is in a valid state.
    pub fn cast<T: Sized>(self) -> &'a T {
        assert_eq!(
            self.ptr.type_info,
            T::TYPE_INFO,
            "Type mismatch: expected {:?}, found {:?}",
            self.ptr.type_info,
            T::TYPE_INFO
        );
        unsafe { &*(self.ptr.data.as_ptr() as *const T) }
    }
}

/// a mutable reference for ErasedDataType
#[repr(transparent)]
#[derive(Debug)]
pub struct ErasedMut<'a> {
    ptr: ErasedMutPointer,
    _phantom: PhantomData<&'a mut ()>,
}

impl<'a> From<ErasedMut<'a>> for ErasedRef<'a> {
    fn from(value: ErasedMut) -> Self {
        ErasedRef {
            ptr: value.ptr,
            _phantom: PhantomData,
        }
    }
}

impl<'a> ErasedMut<'a> {
    /// Safety, this function will compare the layouts of the objects and panic if they don't match
    /// It's up to the user to cast back to the right type
    /// However, this abstraction assume that the pointed value is in a valid state.
    pub fn cast<T: Sized>(self) -> &'a mut T {
        assert_eq!(
            self.ptr.type_info,
            T::TYPE_INFO,
            "Type mismatch: expected {:?}, found {:?}",
            self.ptr.type_info,
            T::TYPE_INFO
        );
        unsafe { &mut *(self.ptr.data.as_ptr() as *mut T) }
    }

    /// replace the contained value with
    pub fn write(&mut self, drop_location: DropLocation) {
        unsafe {
            self.ptr.drop_in_place();
            self.ptr.write_drop_location(drop_location);
        }
    }
}

#[test]
fn ensure_layout() {
    use std::alloc::Layout;
    assert_eq!(
        Layout::new::<ErasedMutPointer>(),
        Layout::new::<Option<ErasedMutPointer>>()
    )
}
