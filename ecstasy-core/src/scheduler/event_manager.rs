use std::{cell::UnsafeCell, collections::HashMap};

use ecstasy_ffi::{self, EventIndex, PublisherVtableExt, TypeDescriptor, TypeIdentity};
use reflexion::{drop_location::DropLocation, erased::ErasedMutPointer, typeinfo::TypeInfo};

use crate::index_storage::IndexStorage;

#[derive(Default)]
pub struct EventManager {
    map: HashMap<TypeIdentity, EventIndex>,
    events: IndexStorage<EventIndex, UnsafeCell<EreasedVec>>,
}

impl EventManager {
    pub fn find_event(&mut self, descriptor: TypeDescriptor) -> EventIndex {
        let v = self.map.entry(descriptor.identity).or_insert_with(|| {
            self.events
                .push(UnsafeCell::new(EreasedVec::new(descriptor.type_info)))
        });
        *v
    }

    pub unsafe fn get_unchecked_publisher(
        &self,
        id: EventIndex,
    ) -> ecstasy_ffi::PublisherHandle<'_> {
        unsafe {
            let vec = &mut *self.events[id].get();
            vec.as_handle()
        }
    }
}

struct EreasedVec {
    data: ErasedMutPointer,
    len: usize,
    capacity: usize,
}

/// a storage for ea
impl EreasedVec {
    pub fn new(type_info: TypeInfo) -> Self {
        Self {
            data: ErasedMutPointer::dangling(type_info),
            len: 0,
            capacity: 0,
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// ensure len < capacity
    fn grow(&mut self, additional: usize) {
        let new_size = self.capacity + additional;
        let len = self.len;
        unsafe {
            if len == 0 {
                self.data.allocate(new_size);
            } else {
                self.data.reallocate(new_size);
            }
        }
    }

    pub fn push(&mut self, value: DropLocation) {
        if self.len() == self.capacity() {
            self.grow(self.capacity().max(4))
        }
        unsafe {
            self.data.offset(self.len()).write_drop_location(value);
        }
    }
}

impl ecstasy_ffi::Publisher for EreasedVec {
    extern "C" fn push(&mut self, value: DropLocation) {
        self.push(value);
    }
}
