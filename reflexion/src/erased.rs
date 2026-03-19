//! Provide a bunch on erased handle following rust goods manners
//! Most of the building blocks of this module are equivalents of mutable pointers, references and mutables references
//! when the value isn't known at compile time

use crate::typeinfo::{TypeInfo, TypeInfoProvider};
use std::marker::PhantomData;
use std::mem;

/// A pointer encapsulation without any type information.
/// This is used to store pointers to any type in a generic way.
/// It can be viewed as a wide pointer that carries a reference to the type information of the pointed type.

/// This is still a fairly low level abstraction, this doesn't really care if it content have been initialized
/// or not, therefor, it doesn't perform any kind of ownership
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ErasedMutPointer {
    pub data: *mut u8,
    pub type_info: TypeInfo,
}

impl ErasedMutPointer {
    pub fn null(type_info: TypeInfo) -> Self {
        ErasedMutPointer {
            type_info,
            data: std::ptr::null_mut(),
        }
    }

    pub unsafe fn from_mut<T: Sized>(data: &mut T) -> Self {
        ErasedMutPointer {
            type_info: T::TYPE_INFO,
            data: data as *mut T as *mut u8,
        }
    }

    pub fn is_null(self) -> bool {
        self.data.is_null()
    }

    pub fn set_null(&mut self) {
        self.data = std::ptr::null_mut();
    }

    /// allocate a memory block
    pub unsafe fn allocate(&mut self, count: usize) {
        self.data = if self.type_info.layout.size == 0 {
            std::ptr::dangling_mut()
        } else {
            unsafe {
                std::alloc::alloc(std::alloc::Layout::from_size_align_unchecked(
                    self.type_info.layout.size * count,
                    self.type_info.layout.align,
                ))
            }
        };
    }

    /// reallocate a memory block
    pub unsafe fn reallocate(&mut self, new_count: usize) {
        self.data = if self.type_info.layout.size == 0 {
            std::ptr::dangling_mut()
        } else {
            unsafe {
                std::alloc::realloc(
                    self.data,
                    std::alloc::Layout::from_size_align_unchecked(
                        self.type_info.layout.size,
                        self.type_info.layout.align,
                    ),
                    self.type_info.layout.size * new_count,
                )
            }
        };
    }

    /// free the associated memory block.
    pub unsafe fn deallocate(self, count: usize) {
        if self.type_info.layout.size == 0 {
            return; // no need to deallocate zero-sized types
        }
        unsafe {
            std::alloc::dealloc(
                self.data,
                std::alloc::Layout::from_size_align_unchecked(
                    self.type_info.layout.size * count,
                    self.type_info.layout.align,
                ),
            );
        }
    }

    /// offset the pointer using the stored type size.
    pub unsafe fn offset(self, offset: usize) -> Self {
        unsafe {
            ErasedMutPointer {
                type_info: self.type_info,
                data: self
                    .data
                    .offset((offset * self.type_info.layout.size) as isize),
            }
        }
    }

    pub unsafe fn copy_nonoverlapping_from(&self, source: ErasedMutPointer) {
        assert_eq!(
            self.type_info, source.type_info,
            "Type mismatch: cannot copy data of type {} to location of type {}",
            source.type_info.layout.size, self.type_info.layout.size
        );
        assert!(!source.is_null(), "Cannot copy from a null pointer");
        unsafe {
            std::ptr::copy_nonoverlapping(source.data, self.data, self.type_info.layout.size);
        }
    }

    pub unsafe fn drop_in_place(self) {
        unsafe {
            (self.type_info.destructor)(self.data);
        }
    }

    pub unsafe fn read<T>(self) -> T {
        unsafe { (self.data as *const T).read() }
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
            std::ptr::write(self.data as *mut T, src)
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
        unsafe { &*(self.ptr.data as *const T) }
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
        unsafe { &mut *(self.ptr.data as *mut T) }
    }

    /// replace the contained value with
    pub fn write(&mut self, drop_location: DropLocation) {
        unsafe {
            self.ptr.drop_in_place();
            self.ptr.write_drop_location(drop_location);
        }
    }
}

/// A place where a thing is about to be dropped. If nothing is done, the underlying value is dropped.
#[repr(C)]
pub struct DropLocation<'a> {
    location: ErasedMutPointer,
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
