//! Provide a bunch on erased handle following rust goods manners
//! Most of the building blocks of this module are equivalents of mutable pointers, references and mutables references
//! when the value isn't known at compile time

use crate::{
    drop_location::DropLocation,
    typeinfo::{TypeInfo, TypeInfoProvider},
};
use std::{fmt::Debug, marker::PhantomData, mem, ptr::NonNull, slice};

/// a Trait used to define
pub unsafe trait ZeroSized {}

#[repr(C)]
pub struct Any {
    _data: (),
    _marker: core::marker::PhantomData<(*mut u8, core::marker::PhantomPinned)>,
}

unsafe impl ZeroSized for Any {}

/// A pointer encapsulation without any type information.
/// This is used to store pointers to any type in a generic way.
/// It can be viewed as a wide pointer that carries a reference to the type information of the pointed type.

/// This is still a fairly low level abstraction, this doesn't really care if it content have been initialized
/// or not, therefor, it doesn't perform any kind of ownership
/// It is equivalent of a NonNull ptr type.
#[repr(C)]
pub struct ErasedMutPointer<PTR: ZeroSized = Any> {
    pub data: NonNull<u8>,
    pub type_info: TypeInfo,
    phantom: PhantomData<PTR>,
}

impl<PTR: ZeroSized> Clone for ErasedMutPointer<PTR> {
    fn clone(&self) -> Self {
        Self {
            data: self.data.clone(),
            type_info: self.type_info.clone(),
            phantom: PhantomData,
        }
    }
}

impl<PTR: ZeroSized> Copy for ErasedMutPointer<PTR> {}

impl<PTR: ZeroSized> Debug for ErasedMutPointer<PTR> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ErasedMutPointer")
            .field("data", &self.data)
            .field("type_info", &self.type_info)
            .finish()
    }
}

impl<PTR: ZeroSized> ErasedMutPointer<PTR> {
    /// return a pointer set to null, without any type associated
    /// any operation on the pointer will fail
    pub fn empty() -> Self {
        Self::dangling(None)
    }

    pub fn dangling(type_info: TypeInfo) -> Self {
        Self {
            type_info,
            data: NonNull::dangling(),
            phantom: PhantomData,
        }
    }

    pub unsafe fn from_mut<T: Sized>(data: &mut T) -> Self {
        unsafe {
            Self {
                type_info: T::TYPE_INFO,
                data: NonNull::new_unchecked(data as *mut T as *mut u8),
                phantom: PhantomData,
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
                phantom: PhantomData,
            }
        }
    }

    /// perform a ref cast but only for zero sized type
    pub unsafe fn as_zeros_sized_ref<'a>(self) -> &'a PTR {
        unsafe {
            let ptr = self.data.as_ptr() as *const PTR;
            &*ptr
        }
    }

    /// perform a mutable ref cast but only for zero sized type
    pub unsafe fn as_zeros_sized_mut<'a>(self) -> &'a mut PTR {
        unsafe {
            let ptr = self.data.as_ptr() as *mut PTR;
            &mut *ptr
        }
    }

    pub unsafe fn as_slice<'a, T>(self, len: usize) -> &'a [T] {
        unsafe {
            let data = self.data.as_ptr() as *const T;
            slice::from_raw_parts::<T>(data, len)
        }
    }

    pub unsafe fn copy_nonoverlapping_from(&self, source: ErasedMutPointer<PTR>, len: usize) {
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
                type_info.layout.size * len,
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

    pub unsafe fn read<T: Sized>(self) -> T {
        unsafe { (self.data.as_ptr() as *const T).read() }
    }

    pub unsafe fn write<T: Sized>(self, src: T) {
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

    /// move the content from a drop location and discard it
    pub unsafe fn write_drop_location(self, location: DropLocation<PTR>) {
        unsafe { self.copy_nonoverlapping_from(location.location, 1) }
        mem::forget(location);
    }

    /// build a reference, the lifetime should be provided by the caller
    pub unsafe fn as_erased_ref<'a>(self) -> ErasedRef<'a, PTR> {
        ErasedRef {
            ptr: self,
            _phantom: PhantomData,
        }
    }

    /// build a reference, the lifetime should be provided by the caller
    pub unsafe fn as_erased_mut<'a>(self) -> ErasedMut<'a, PTR> {
        ErasedMut {
            ptr: self,
            _phantom: PhantomData,
        }
    }
}

/// a reference for ErasedDataType
#[repr(transparent)]
pub struct ErasedRef<'a, PTR: ZeroSized = Any> {
    ptr: ErasedMutPointer<PTR>,
    _phantom: PhantomData<&'a ()>,
}

impl<'a, PTR: ZeroSized> Clone for ErasedRef<'a, PTR> {
    fn clone(&self) -> Self {
        Self {
            ptr: self.ptr.clone(),
            _phantom: self._phantom.clone(),
        }
    }
}

impl<'a, PTR: ZeroSized> Copy for ErasedRef<'a, PTR> {}

impl<'a, PTR: ZeroSized> ErasedRef<'a, PTR> {
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
pub struct ErasedMut<'a, PTR: ZeroSized = Any> {
    ptr: ErasedMutPointer<PTR>,
    _phantom: PhantomData<&'a mut ()>,
}

impl<'a, PTR: ZeroSized> From<ErasedMut<'a, PTR>> for ErasedRef<'a, PTR> {
    fn from(value: ErasedMut<PTR>) -> Self {
        ErasedRef {
            ptr: value.ptr,
            _phantom: PhantomData,
        }
    }
}

impl<'a, PTR: ZeroSized> ErasedMut<'a, PTR> {
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
    pub fn write(&mut self, drop_location: DropLocation<PTR>) {
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
