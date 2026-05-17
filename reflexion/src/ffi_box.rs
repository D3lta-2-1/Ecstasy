use core::alloc;
use std::{
    alloc::{alloc, dealloc},
    ptr::write,
};

use crate::{
    opaque::Opaque,
    typeinfo::{Layout, TypeInfo, TypeInfoImpl, TypeInfoProvider},
};

#[repr(C)]
pub struct FfiBox<T: Opaque>
where
    <T as Opaque>::Vtable: 'static,
{
    typeinfo: TypeInfo,
    data: *mut T,
    vtable: &'static T::Vtable,
    // SAFETY: we are forced to keep the memory release function with us because the allocation and free could not happen in the same binary
    free_memory: unsafe extern "C" fn(*mut u8, Layout),
}

impl<T: Opaque> Drop for FfiBox<T> {
    fn drop(&mut self) {
        let TypeInfoImpl { layout, destructor } = self.typeinfo.unwrap();
        unsafe {
            destructor(self.data as *mut u8);
            (self.free_memory)(self.data as *mut u8, *layout)
        }
    }
}

/// reexport for ABI safety
unsafe extern "C" fn free_memory(data: *mut u8, layout: Layout) {
    unsafe {
        dealloc(data, layout.into());
    }
}

impl<T: Opaque> FfiBox<T> {
    /// this function is marked as unsafe because the compiler can't check weather or not the type passed really implement the matching trait
    pub unsafe fn new<U: TypeInfoProvider>(object: U, vtable: &'static T::Vtable) -> Self {
        unsafe {
            let data: *mut U = alloc(alloc::Layout::new::<U>()) as _;
            write(data, object);
            Self {
                typeinfo: U::TYPE_INFO,
                data: data as _,
                vtable,
                free_memory: free_memory,
            }
        }
    }

    pub fn handle<'a>(&'a self) -> T::Handle<'a> {
        unsafe { T::handle(self.data as _, self.vtable) }
    }

    pub fn mut_handle<'a>(&'a mut self) -> T::MutHandle<'a> {
        unsafe { T::mut_handle(self.data as _, self.vtable) }
    }
}
