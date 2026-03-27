use super::component_bridge::ComponentIdentityBridge;
use super::{ColumnIndex, EntityIndex};
use crate::shared::id::{Component, Entity};
use reflexion::erased::{DropLocation, ErasedMut, ErasedMutPointer, ErasedRef};
use std::alloc::{Layout, handle_alloc_error};
use std::fmt::{Debug, Formatter};
use std::iter::zip;

/// structure in charge of storing data for a specific entity
pub struct Archetype {
    components: Vec<Component>, // column_descriptor must be sorted
    columns: Vec<ErasedMutPointer>,
    entities: Vec<Entity>,
}

impl Debug for Archetype {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "component header {:?}", self.components)?;
        writeln!(f, "entity stored {:?}", self.len())
    }
}

impl Archetype {
    /// build a new archetype from a set of Column, the components needs to be sorted
    pub fn new(components: Vec<Entity>, component_bridge: &ComponentIdentityBridge) -> Self {
        debug_assert!(components.is_sorted(), "components needs to be sorted...");
        let columns: Vec<_> = components
            .iter()
            .map(|component| {
                let info = component_bridge.find_type_info(component);
                ErasedMutPointer::null(info)
            })
            .collect();

        Archetype {
            entities: Vec::new(),
            columns,
            components,
        }
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
        unsafe {
            for columm in self.columns.iter_mut() {
                if columm.is_null() {
                    columm.allocate(new_size);
                } else {
                    columm.reallocate(new_size);
                }
                if columm.is_null() {
                    handle_alloc_error(
                        Layout::from_size_align(
                            columm.type_info.layout.size * new_size,
                            columm.type_info.layout.align,
                        )
                        .unwrap(),
                    )
                }
            }
        }
        self.entities.reserve(additional);
    }

    /// add a new entity, all DropLocation must remain valid until the end of the call
    pub fn push<'a>(
        &mut self,
        id: Entity,
        components: impl Iterator<Item = (Component, DropLocation<'a>)>,
    ) -> Result<EntityIndex, ArchetypeError> {
        if self.len() == self.capacity() {
            self.grow_columns(self.capacity().max(4))
        }

        let location = self.len();
        let columns = zip(self.components.iter().cloned(), self.columns.iter_mut());

        //the Component information is useless here, but at least it can guaranty that things went smoothly
        for (i, ((given_component, value), (expected_component, column))) in
            zip(components, columns).enumerate()
        {
            if given_component != expected_component {
                // drop any failed init...
                for column in self.columns[0..i].iter().cloned() {
                    unsafe { column.offset(location).drop_in_place() }
                }
                return Err(ArchetypeError::InsertionFailed);
            }
            unsafe {
                column.offset(location).write_drop_location(value);
            }
        }
        self.entities.push(id);
        Ok(location)
    }

    pub fn ref_at<'a>(&'a self, column: ColumnIndex, index: EntityIndex) -> ErasedRef<'a> {
        assert!(index < self.len(), "out of range");
        unsafe { self.columns[column].offset(index).as_erased_ref::<'a>() }
    }

    pub fn mut_at<'a>(&'a mut self, column: ColumnIndex, index: EntityIndex) -> ErasedMut<'a> {
        assert!(index < self.len(), "out of range");
        unsafe { self.columns[column].offset(index).as_erased_mut::<'a>() }
    }

    /// return an iterator containing all removed components
    pub fn swap_remove<'a>(&'a mut self, location: EntityIndex) -> RemoveIterator<'a> {
        RemoveIterator::<'a>::new(self, location)
    }
}

impl Drop for Archetype {
    fn drop(&mut self) {
        unsafe {
            for column in self.columns.iter().cloned() {
                for i in 0..self.len() {
                    column.offset(i).drop_in_place();
                }
                column.deallocate(self.capacity());
            }
        }
    }
}

pub struct RemoveIterator<'a> {
    archetype: &'a mut Archetype,
    i: usize,
    location: EntityIndex,
}

impl<'a> RemoveIterator<'a> {
    fn new(archetype: &'a mut Archetype, location: EntityIndex) -> Self {
        Self {
            archetype,
            i: 0,
            location,
        }
    }

    ///return which entity is being moved, and where it will end up
    pub fn moved_entity(&self) -> Option<(Entity, EntityIndex)> {
        if self.archetype.len() > 1 {
            self.archetype.entities.last().map(|e| (*e, self.location))
        } else {
            None
        }
    }
}

impl<'a> Iterator for RemoveIterator<'a> {
    type Item = (Entity, DropLocation<'a>);

    fn next(&mut self) -> Option<Self::Item> {
        if self.i >= self.archetype.columns.len() {
            return None;
        }

        let value = unsafe {
            let location = self.archetype.columns[self.i].offset(self.location);
            (
                self.archetype.components[self.i],
                DropLocation::at(location),
            )
        };
        self.i += 1;
        Some(value)
    }
}

impl<'a> ExactSizeIterator for RemoveIterator<'a> {
    fn len(&self) -> usize {
        self.archetype.columns.len()
    }
}

impl<'a> Drop for RemoveIterator<'a> {
    fn drop(&mut self) {
        while self.next().is_some() {} // drop all remaining elements
        let len = self.archetype.len();
        if len > 1 {
            for column in self.archetype.columns[0..self.i].iter().cloned() {
                unsafe {
                    column
                        .offset(self.location)
                        .copy_nonoverlapping_from(column.offset(len))
                };
            }
        }
        self.archetype.entities.swap_remove(self.location);
    }
}

#[derive(Debug)]
pub enum ArchetypeError {
    InsertionFailed,
}
