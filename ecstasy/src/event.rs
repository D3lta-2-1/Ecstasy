use std::marker::PhantomData;

use ecstasy_ffi::{PublisherHandle, TypeDescriptor, TypeIdentity};
use reflexion::{drop_location::DropLocation, ffi_slice::FfiSlice, typeinfo::TypeInfoProvider};

pub trait Event: TypeInfoProvider {
    const PATH: &'static str;
    const NAME: &'static str;
    const DESCRIPTOR: TypeDescriptor = TypeDescriptor {
        identity: TypeIdentity {
            path: FfiSlice::from_str(Self::PATH),
            name: FfiSlice::from_str(Self::NAME),
        },
        type_info: Self::TYPE_INFO,
    };
}

pub struct Publisher<'publisher, T: Event> {
    pub(crate) inner: PublisherHandle<'publisher>,
    pub(crate) phantom: PhantomData<T>,
}

impl<'publisher, T: Event> Publisher<'publisher, T> {
    pub fn emit(&mut self, mut value: T) {
        unsafe {
            let location = DropLocation::at_hard(&mut value);
            (self.inner.vtable.push)(self.inner.handle, location);
        }
    }
}
