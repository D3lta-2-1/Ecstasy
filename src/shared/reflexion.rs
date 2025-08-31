use std::hash::Hash;
use std::mem::forget;
use crate::shared::id::Entity;

/// we redefine the `Layout` in order to make it C-compatible.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Layout {
    pub size: usize,
    pub align: usize,
}

impl Layout {
    pub const fn new<T>() -> Self {
        Layout {
            size: size_of::<T>(),
            align: align_of::<T>(),
        }
    }
}

impl From<std::alloc::Layout> for Layout {
    fn from(layout: std::alloc::Layout) -> Self {
        Layout {
            size: layout.size(),
            align: layout.align(),
        }
    }
}

impl From<Layout>  for std::alloc::Layout {
    fn from(value: Layout) -> Self {
        std::alloc::Layout::from_size_align(value.size, value.align)
            .expect("Invalid layout: size and align must be valid")
    }
}


/// This module tries to provide a way to access type information at runtime.
/// each type will have a `TypeInfo` structure that contains its layout and a destructor function.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct TypeInfo {
    pub layout: Layout,
    pub destructor: unsafe fn(*mut u8),
}

impl PartialEq for TypeInfo {
    fn eq(&self, other: &Self) -> bool {
        self.layout == other.layout
    }
}

impl Eq for TypeInfo {}

impl Hash for TypeInfo {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.layout.hash(state);
    }
}

impl TypeInfo {
    pub unsafe fn destructor<T>(to_drop: *mut u8) {
        unsafe {
            std::ptr::drop_in_place(to_drop);
        }
    }

    pub const EMPTY: &'static TypeInfo = <()>::TYPE_INFO;
}

pub trait TypeInfoProvider {
    const TYPE_INFO: &'static TypeInfo;
}



impl<T: Sized> TypeInfoProvider for T {
    const TYPE_INFO: &'static TypeInfo = &TypeInfo {
        layout: Layout::new::<T>(),
        destructor: TypeInfo::destructor::<T>,
    };
}

/// A pointer encapsulation without any type information.
/// This is used to store pointers to any type in a generic way.
/// It can be viewed as a wide pointer that carries a reference to the type information of the pointed type.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ErasedPointer {
    pub data: *mut u8,
    pub type_info: &'static TypeInfo,
}

impl ErasedPointer {
    pub fn from_type_info(type_info: &'static TypeInfo) -> Self {
        ErasedPointer {
            type_info,
            data: std::ptr::null_mut(),
        }
    }

    pub unsafe fn create_ref<T: Sized>(data: &mut T) -> Self {
        ErasedPointer {
            type_info: T::TYPE_INFO,
            data: data as *mut T as *mut u8,
        }
    }

    pub fn is_null(&self) -> bool {
        self.data.is_null()
    }

    pub fn set_null(&mut self) {
        self.data = std::ptr::null_mut();
    }

    pub unsafe fn allocate(type_info: &'static TypeInfo, count: usize) -> Self {
        let data = if type_info.layout.size == 0 {
            std::ptr::dangling_mut()
        } else {
            std::alloc::alloc(std::alloc::Layout::from_size_align_unchecked(type_info.layout.size * count, type_info.layout.align))
        };
        ErasedPointer {
            type_info,
            data,
        }
    }

    pub unsafe fn reallocate(&self, new_count: usize) -> Self {
        let new_data = if self.type_info.layout.size == 0 {
            std::ptr::dangling_mut()
        } else {
            std::alloc::realloc(self.data, std::alloc::Layout::from_size_align_unchecked(self.type_info.layout.size, self.type_info.layout.align), self.type_info.layout.size * new_count)
        };
        ErasedPointer {
            type_info: self.type_info,
            data: new_data,
        }
    }

    pub unsafe fn deallocate(&self, count: usize) {
        if self.type_info.layout.size == 0 {
            return; // no need to deallocate zero-sized types
        }
        std::alloc::dealloc(self.data, std::alloc::Layout::from_size_align_unchecked(self.type_info.layout.size * count, self.type_info.layout.align));
    }

    pub unsafe fn offset(&self, offset: usize) -> Self {
        ErasedPointer {
            type_info: self.type_info,
            data: self.data.offset((offset * self.type_info.layout.size) as isize),
        }
    }

    pub unsafe fn copy_nonoverlapping_from(&self, source: &ErasedPointer) {
        assert_eq!(self.type_info, source.type_info, "Type mismatch: cannot copy data of type {} to location of type {}", source.type_info.layout.size, self.type_info.layout.size);
        assert!(!source.is_null(), "Cannot copy from a null pointer");
        std::ptr::copy_nonoverlapping(source.data, self.data, self.type_info.layout.size);
    }

    pub unsafe fn drop_in_place(&self) {
        (self.type_info.destructor)(self.data);
    }

    pub unsafe fn as_ref<T: Sized>(&self) -> &T {
        assert_eq!(self.type_info, T::TYPE_INFO, "Type mismatch: expected {}, found {}", self.type_info, T::TYPE_INFO);
        &*(self.data as *const T)
    }
}


#[repr(C)]
/// A wrapper around a pointer type that indicate that this pointer is the location of an element that **MUST** be consumed. the taker is responsible for the destruction of that element.
/// Copy of this struct is **STRICTLY PROHIBITED**.
/// used to place component in the archetype columns.
/// if the data wasn't consumed, it will be dropped at the end of this struct lifetime.
pub struct SourceData {
    component_id: Entity, // the component id of the source data, used to ensure that the data is written to the correct location
    address: ErasedPointer,
}

impl SourceData {
    pub fn new<T: Sized>(mut data: T, component_id: Entity, inserter: impl FnOnce(&mut[Self])) {
        unsafe {
            let mut source_data = [SourceData {
                component_id,
                address: ErasedPointer::create_ref(&mut data),
            }];
            inserter(source_data.as_mut_slice());
        }
        forget(data); // prevent the data from being dropped, since it will be moved to the target location
    }

    /// It's like doing a bunch of "read" operations on these pointers, the pointed location should be now considered as consumed.
    /// The `SourceData object created have to leave as long as the target location.
    pub unsafe fn from_erased_pointer(component_id: Entity, address: ErasedPointer) -> Self {
        assert!(!address.is_null(), "Cannot create SourceData from a null pointer");
        SourceData {
            component_id,
            address,
        }
    }

    pub fn get_component_id(&self) -> Entity {
        self.component_id
    }

    pub unsafe fn write_to<T: Sized>(&mut self, location: ErasedPointer) {
        assert_eq!(self.address.type_info, location.type_info, "Type mismatch: cannot write data of type {} to location of type {}", self.address.type_info.layout.size, location.type_info.layout.size);
        assert!(!self.address.is_null(), "the source data as already been consumed, cannot write to the target location");
        std::ptr::copy_nonoverlapping(self.address.data, location.data, self.address.type_info.layout.size);
        self.address.set_null(); // set the source data to null to prevent double drop
    }
}

impl Drop for SourceData {
    fn drop(&mut self) {
        if !self.address.is_null() {
            unsafe {
                self.address.drop_in_place();
            }
        }
    }
}