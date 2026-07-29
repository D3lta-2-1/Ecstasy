use std::{marker::PhantomData, mem};

use ecstasy_ffi::{ComponentDescriptor, ConsumerOpaque, ProducerOpaque, TypeIdentity};
use reflexion::{drop_location::DropLocation, ffi_slice::FfiSlice, typeinfo::TypeInfoProvider};

use crate::loader::{ConsumerLoader, ProducerLoader};

// TODO: publisher and Loader can be globally loaded like other core objects.
pub trait Event: TypeInfoProvider {
    const PATH: &'static str;
    const NAME: &'static str;
    const DESCRIPTOR: ComponentDescriptor = ComponentDescriptor {
        identity: TypeIdentity {
            path: FfiSlice::from_str(Self::PATH),
            name: FfiSlice::from_str(Self::NAME),
        },
        type_info: Self::TYPE_INFO,
        versioned: false,
    };
}

pub struct Producer<'producer, T: Event> {
    pub(crate) inner: &'producer mut ProducerOpaque,
    pub(crate) phantom: PhantomData<T>,
}

impl<'publisher, T: Event> Producer<'publisher, T> {
    pub fn emit(&mut self, mut value: T) {
        unsafe {
            let location = DropLocation::at_hard(&mut value);
            ProducerLoader::push(self.inner, location);
            mem::forget(value);
        }
    }
}

pub struct Consumer<'consumer, T: Event> {
    pub(crate) inner: &'consumer ConsumerOpaque,
    pub(crate) phantom: PhantomData<T>,
}

impl<'consumer, T: Event> Consumer<'consumer, T> {
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        unsafe { ConsumerLoader::events(self.inner).iter() }
    }
}
