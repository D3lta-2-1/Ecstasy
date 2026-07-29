use crate::registry::component_bridge::ComponentInfo;

use super::component_bridge::ComponentIdentityBridge;
use ecstasy_ffi::{ColumnIndex, Component, Entity, EntityIndex};
use reflexion::{
    drop_location::DropLocation,
    erased::{ErasedMut, ErasedMutPointer, ErasedRef},
};
use std::{
    fmt::{Debug, Formatter},
    iter::{self, zip},
    marker::PhantomData,
    ops::Range,
};

pub enum PushEntry<'a, T: Iterator<Item = DropLocation<'a>>> {
    One(DropLocation<'a>),
    Many(T),
}

/// structure in charge of storing data for a specific set of entities
/// the archetype use two type of indices for column addressing,
/// - "component matching" ids, which is mainly dealt with outside of the archetype implementation
/// - "true column" ids, which correspond the each column used by the archetype, including versioning capabilities
pub struct Archetype {
    components: Vec<Component>, // must be sorted
    columns_index: Vec<usize>, // since a component can have multiples column we need to know where the columns used to store the value of a component are located
    // contain |components| + 1 values, and columns_index[i]..columns_index[i + 1] is the range of columns used to store the values
    versioned_component: Vec<usize>, // which component are versioned, used for ring rotation and the start of a tick
    columns_storage: Vec<ErasedMutPointer>, // the actuals columns
    entities: Vec<Entity>,           // entity ID matching a row of values
}

impl Debug for Archetype {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "component header {:?}", self.components)?;
        writeln!(f, "versioned component {:?}", self.versioned_component)?;
        writeln!(f, "entity stored {:?}", self.len())?;
        writeln!(f, "column index{:?}", self.columns_index)
    }
}

impl Archetype {
    /// build a new archetype from a set of Column, the components needs to be sorted
    pub fn new(
        components: Vec<Entity>,
        component_bridge: &ComponentIdentityBridge,
        versioning_count: usize,
    ) -> Self {
        debug_assert!(components.is_sorted(), "components needs to be sorted...");

        let sub_column_count =
            |info: &ComponentInfo| if info.versioned { versioning_count } else { 1 };

        let columns_storage: Vec<_> = components
            .iter()
            .flat_map(|component| {
                let info = component_bridge.find_type_info(component);
                let sub_column_count = sub_column_count(&info);
                iter::repeat_n(ErasedMutPointer::dangling(info.type_info), sub_column_count)
            })
            .collect();

        let mut acc = 0;
        let columns_index: Vec<_> = iter::once(0)
            .chain(components.iter().map(|component| {
                let info = component_bridge.find_type_info(component);
                acc += sub_column_count(&info);
                acc
            }))
            .collect();

        let versioned_component: Vec<_> = components
            .iter()
            .enumerate()
            .filter_map(|(i, component)| {
                let info = component_bridge.find_type_info(component);
                info.versioned.then_some(i)
            })
            .collect();

        let a = Archetype {
            components,
            columns_index,
            versioned_component,
            columns_storage,
            entities: Vec::new(),
        };

        println!("{:?}", a);
        a
    }

    pub fn is_versioned(&self) -> bool {
        !self.versioned_component.is_empty()
    }

    /// Return all components ids stored in the archetype.
    pub fn get_descriptor(&self) -> &[Component] {
        &self.components
    }

    pub fn len(&self) -> usize {
        self.entities.len()
    }

    pub fn capacity(&self) -> usize {
        self.entities.capacity()
    }

    fn grow_columns(&mut self, additional: usize) {
        let new_size = self.capacity() + additional;
        let len = self.len();
        unsafe {
            for column in self.columns_storage.iter_mut() {
                if len == 0 {
                    column.allocate(new_size);
                } else {
                    column.reallocate(new_size);
                }
            }
        }
        self.entities.reserve(additional);
    }

    /// add a new entity, all DropLocation must remain valid until the end of the call
    /// expect an iterator containing only one value per component (versioning is excluded)
    pub fn push<'a, T: Iterator<Item = DropLocation<'a>>>(
        &mut self,
        id: Entity,
        components: impl Iterator<Item = (Component, PushEntry<'a, T>)>,
    ) -> Result<EntityIndex, ArchetypeError> {
        if self.len() == self.capacity() {
            self.grow_columns(self.capacity().max(4))
        }

        let location = self.len();
        let columns = zip(
            self.components.iter().cloned(),
            self.columns_index.array_windows(),
        )
        .map(|(component, [start, end])| (component, *start..*end));

        //the Component information is useless here, but at least it can guaranty that things went smoothly
        for ((given_component, entry), (expected_component, range)) in zip(components, columns) {
            if given_component != expected_component {
                // drop any failed init...
                for column in self.columns_storage[0..range.start].iter().cloned() {
                    unsafe { column.offset(location).drop_in_place() }
                }
                return Err(ArchetypeError::InsertionFailed);
            }

            match entry {
                // spread the entry one all cells
                PushEntry::One(value) => unsafe {
                    let first = self.columns_storage[range.start].offset(location);
                    first.write_drop_location(value);
                    // SAFETY: if the component isn't copy, it can't be versioned, so the for-loop isn't ran
                    for column in self.columns_storage[range.start + 1..].iter().cloned() {
                        column.offset(location).copy_nonoverlapping_from(first, 1);
                    }
                },
                // move 'olds' entries
                PushEntry::Many(values) => {
                    let mut i = range.start;
                    unsafe {
                        for (j, value) in values.enumerate().take(range.len()) {
                            i = j + range.start;
                            let column = &self.columns_storage[i];
                            column.offset(location).write_drop_location(value);
                        }
                    }
                    if i < range.len() - 1 {
                        // if a component is missing, clean init
                        for column in self.columns_storage[0..i].iter().cloned() {
                            unsafe { column.offset(location).drop_in_place() }
                        }
                        return Err(ArchetypeError::InsertionFailed);
                    }
                }
            }
        }
        self.entities.push(id);
        Ok(EntityIndex(location))
    }

    pub fn ref_at<'a>(&'a self, location: ComponentValueLocation, tick: u64) -> ErasedRef<'a> {
        assert!(location.entity_index.0 < self.len(), "out of range");
        let index = self.compute_column(location.column.0, tick);
        unsafe {
            self.columns_storage[index]
                .offset(location.entity_index.0)
                .as_erased_ref::<'a>()
        }
    }

    pub fn mut_at<'a>(&'a mut self, location: ComponentValueLocation, tick: u64) -> ErasedMut<'a> {
        assert!(location.entity_index.0 < self.len(), "out of range");
        let index = self.compute_column(location.column.0, tick);
        unsafe {
            self.columns_storage[index]
                .offset(location.entity_index.0)
                .as_erased_mut::<'a>()
        }
    }

    /// return an iterator containing all removed components
    pub fn swap_remove<'a>(&'a mut self, location: EntityIndex) -> RemoveIterator<'a> {
        RemoveIterator::<'a>::new(self, location)
    }

    fn compute_column(&self, component_index: usize, tick: u64) -> usize {
        let start = self.columns_index[component_index];
        let end = self.columns_index[component_index + 1];
        let len = end - start;
        start + tick as usize % len
    }

    /// fill the array in reference with the start of the request column, the "n-th" component can be accessed with the ``offset``function
    // TODO: support too old request failure
    pub unsafe fn get_column_begin(
        &self,
        tick: u64,
        columns: &[ColumnIndex],
        starts: &mut [ErasedMutPointer],
    ) -> &[Entity] {
        for (column_start, index) in zip(starts.iter_mut(), columns) {
            let index = self.compute_column(index.0, tick);
            *column_start = self.columns_storage[index];
        }
        &self.entities
    }

    /// roll the versioned components if any, meaning that the newest column content will be copied on the oldest
    pub fn tick(&mut self, new_tick: u64) {
        let old_tick = new_tick - 1;

        for i in self.versioned_component.iter().cloned() {
            let src_index = self.compute_column(i, old_tick);
            let dest_index = self.compute_column(i, old_tick);
            debug_assert_ne!(src_index, dest_index, "the component isn't versioned");

            let len = self.len();
            unsafe {
                let src = self.columns_storage[src_index];
                let dest = self.columns_storage[dest_index];
                // SAFETY: the underlying type must implement Copy, so there is no worries about drop.
                dest.copy_nonoverlapping_from(src, len);
            }
        }
    }
}

pub struct ComponentValueLocation {
    pub column: ColumnIndex,
    pub entity_index: EntityIndex,
}

impl Drop for Archetype {
    fn drop(&mut self) {
        unsafe {
            for column in self.columns_storage.iter().cloned() {
                for i in 0..self.len() {
                    column.offset(i).drop_in_place();
                }
                column.deallocate(self.capacity());
            }
        }
    }
}

/// iterator build when a component is removed from the archetype
/// - if the removed component isn't versioned, this iterator only return one element
pub struct ComponentIterator<'a> {
    archetype: &'a Archetype,
    range: Range<usize>,
    i: usize,
    location: EntityIndex,
}

impl<'a> ComponentIterator<'a> {
    fn new(archetype: &'a Archetype, location: EntityIndex, component: usize) -> Self {
        let start = archetype.columns_index[component];
        let end = archetype.columns_index[component + 1];
        Self {
            archetype,
            range: start..end,
            i: start,
            location,
        }
    }
}

impl<'a> Iterator for ComponentIterator<'a> {
    type Item = DropLocation<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.i >= self.range.end {
            return None;
        }

        let value = unsafe {
            let location = self.archetype.columns_storage[self.i].offset(self.location.0);
            DropLocation::at(location)
        };
        self.i += 1;
        Some(value)
    }
}

impl<'a> Drop for ComponentIterator<'a> {
    fn drop(&mut self) {
        while let Some(_) = self.next() {}
    }
}

pub struct RemoveIterator<'a> {
    archetype: *mut Archetype,
    phantom: PhantomData<&'a mut ()>,
    processed_component: usize,
    location: EntityIndex,
}

impl<'a> RemoveIterator<'a> {
    fn new(archetype: &'a mut Archetype, location: EntityIndex) -> Self {
        Self {
            archetype,
            phantom: PhantomData,
            processed_component: 0,
            location,
        }
    }

    ///return which entity is being moved, and where it will end up
    pub fn moved_entity(&self) -> Option<(Entity, EntityIndex)> {
        unsafe {
            if (*self.archetype).len() > 1 {
                (*self.archetype)
                    .entities
                    .last()
                    .map(|e| (*e, self.location))
            } else {
                None
            }
        }
    }
}

impl<'a> Iterator for RemoveIterator<'a> {
    type Item = (Entity, PushEntry<'a, ComponentIterator<'a>>);

    fn next(&mut self) -> Option<Self::Item> {
        unsafe {
            if self.processed_component >= (*self.archetype).components.len() {
                return None;
            }

            let entity: Entity = (&*self.archetype).components[self.processed_component];

            let value = (
                entity,
                PushEntry::Many(ComponentIterator::<'a>::new(
                    &*self.archetype,
                    self.location,
                    self.processed_component,
                )),
            );
            self.processed_component += 1;
            Some(value)
        }
    }
}

impl<'a> ExactSizeIterator for RemoveIterator<'a> {
    fn len(&self) -> usize {
        unsafe { (*self.archetype).columns_storage.len() }
    }
}

impl<'a> Drop for RemoveIterator<'a> {
    fn drop(&mut self) {
        unsafe {
            while self.next().is_some() {} // drop all remaining elements
            let len = (*self.archetype).len();
            if len > 1 {
                for column in (&*self.archetype).columns_storage[0..self.processed_component]
                    .iter()
                    .cloned()
                {
                    column
                        .offset(self.location.0)
                        .copy_nonoverlapping_from(column.offset(len), 1)
                }
            }
            (*self.archetype).entities.swap_remove(self.location.0);
        }
    }
}

#[derive(Debug)]
pub enum ArchetypeError {
    InsertionFailed,
}
