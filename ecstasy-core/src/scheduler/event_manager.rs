use std::{cell::UnsafeCell, collections::HashMap};

use ecstasy_ffi::{
    self, ComponentDescriptor, ConsumerOpaque, ConsumerVtable, ConsumerVtableExt, EventIndex,
    ProducerOpaque, ProducerVtable, ProducerVtableExt, TypeIdentity,
};
use reflexion::{drop_location::DropLocation, erased::ErasedMutPointer, typeinfo::TypeInfo};

use crate::index_storage::IndexStorage;

#[derive(Default)]
pub struct EventManager {
    map: HashMap<TypeIdentity, EventIndex>,
    events: IndexStorage<EventIndex, UnsafeCell<EreasedVec>>,
}

impl EventManager {
    pub fn clear(&mut self) {
        for storage in self.events.values_mut() {
            storage.get_mut().clear();
        }
    }

    pub fn find_event(&mut self, descriptor: ComponentDescriptor) -> EventIndex {
        let v = self.map.entry(descriptor.identity).or_insert_with(|| {
            self.events
                .push(UnsafeCell::new(EreasedVec::new(descriptor.type_info)))
        });
        *v
    }

    pub unsafe fn get_unchecked_publisher(&self, id: EventIndex) -> &mut ProducerOpaque {
        unsafe {
            let vec = &mut *self.events[id].get();
            <EreasedVec as ProducerVtableExt>::as_opaque_mut(vec)
        }
    }

    pub unsafe fn get_unchecked_consumer(&self, id: EventIndex) -> &ConsumerOpaque {
        unsafe {
            let vec = &*self.events[id].get();
            <EreasedVec as ConsumerVtableExt>::as_opaque(vec)
        }
    }
}

pub struct EreasedVec {
    data: ErasedMutPointer,
    len: usize,
    capacity: usize,
}

/// a storage for each event
impl EreasedVec {
    pub fn new(type_info: TypeInfo) -> Self {
        Self {
            data: ErasedMutPointer::dangling(type_info),
            len: 0,
            capacity: 0,
        }
    }

    pub fn clear(&mut self) {
        if self.data.type_info.is_some_and(|info| info.need_drop) {
            for i in 0..self.len() {
                unsafe {
                    self.data.offset(i).drop_in_place();
                }
            }
        }
        self.len = 0;
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

pub const PRODUCER_VTABLE: &'static ProducerVtable = &<EreasedVec as ProducerVtableExt>::VTABLE;
pub const CONSUMER_VTABLE: &'static ConsumerVtable = &<EreasedVec as ConsumerVtableExt>::VTABLE;

impl ecstasy_ffi::Producer for EreasedVec {
    unsafe extern "C-unwind" fn push(&mut self, value: DropLocation) {
        self.push(value);
    }
}

impl ecstasy_ffi::Consumer for EreasedVec {
    unsafe extern "C-unwind" fn events(&self, len: &mut usize) -> ErasedMutPointer {
        *len = self.len();
        self.data
    }
}
