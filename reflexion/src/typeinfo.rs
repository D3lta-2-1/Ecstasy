//! This module tries to provide a way to access type information at runtime, in an ABI-sage manner

use std::hash::Hash;

/// Redefinition of the `core::alloc::Layout` in order to stabilize it's ABI
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

impl From<Layout> for std::alloc::Layout {
    fn from(value: Layout) -> Self {
        unsafe { std::alloc::Layout::from_size_align_unchecked(value.size, value.align) }
    }
}

/// Store enough information
/// this type should always be passed using ``&'static TypeInfo``
#[repr(C)]
#[derive(Debug)]
pub struct TypeInfoImpl {
    pub layout: Layout,
    pub destructor: unsafe fn(*mut u8),
}

pub type TypeInfo = &'static TypeInfoImpl;

impl PartialEq for TypeInfoImpl {
    fn eq(&self, other: &Self) -> bool {
        self.layout == other.layout
    }
}

impl Eq for TypeInfoImpl {}

impl Hash for TypeInfoImpl {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.layout.hash(state);
    }
}

impl TypeInfoImpl {
    pub unsafe fn destructor<T>(to_drop: *mut u8) {
        unsafe {
            std::ptr::drop_in_place(to_drop);
        }
    }

    pub const EMPTY: TypeInfo = <()>::TYPE_INFO;
}

/// Used to associate each type to its matching ``TypeInfo``
pub trait TypeInfoProvider {
    const TYPE_INFO: TypeInfo;
}

impl<T: Sized> TypeInfoProvider for T {
    const TYPE_INFO: TypeInfo = &TypeInfoImpl {
        layout: Layout::new::<T>(),
        destructor: TypeInfoImpl::destructor::<T>,
    };
}
