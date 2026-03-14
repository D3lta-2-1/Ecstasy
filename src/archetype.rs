use crate::component_bridge::ComponentIdentityBridge;
use crate::shared::id::{Component, Entity};
use reflexion::erased::{DropLocation, ErasedMutPointer};
use std::alloc::{Layout, handle_alloc_error};
use std::collections::HashMap;
use std::fmt::{Debug, Formatter};
use std::iter::zip;

/// structure in charge of storing data for a specific entity
pub struct Archetype {
    components: Vec<Component>, // column_descriptor must be sorted
    columns: Vec<ErasedMutPointer>,
    entities: Vec<Entity>,
    components_to_columns: HashMap<Entity, usize>, // maps component id to column index
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
        assert!(components.is_sorted(), "components needs to be sorted...");
        let columns: Vec<_> = components
            .iter()
            .map(|component| {
                let info = component_bridge.find_type_info(component);
                ErasedMutPointer::null(info.type_info)
            })
            .collect();

        let components_to_columns: HashMap<_, _> = components
            .iter()
            .cloned()
            .enumerate()
            .map(|(i, component)| (component, i))
            .collect();

        Archetype {
            entities: Vec::new(),
            columns,
            components,
            components_to_columns,
        }
    }

    /// Return all components ids stored in the archetype.
    pub fn get_descriptor(&self) -> &Vec<Component> {
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
        components: impl ExactSizeIterator<Item = (Component, DropLocation<'a>)>,
    ) -> usize {
        assert_eq!(
            components.len(),
            self.components.len(),
            "try to push with the wrong amount of components"
        );
        if self.len() == self.capacity() {
            self.grow_columns(self.capacity())
        }

        let location = self.len();
        let columns = zip(self.components.iter().cloned(), self.columns.iter_mut());

        //the Component information is useless here, but at least it can guaranty that things went smoothly
        for ((given_component, value), (expected_component, column)) in zip(components, columns) {
            assert_eq!(given_component, expected_component, "wrong component found");
            //TODO: return a result depending weather or not the push was a success instead of a crash ?
            unsafe {
                column.offset(location).write_drop_location(value);
            }
        }
        self.entities.push(id);
        location
    }

    /// return an iterator containing all removed components
    pub fn swap_remove(&mut self, location: usize) -> RemoveIterator {
        RemoveIterator::new(self, location)
    }
}

pub struct RemoveIterator<'a> {
    archetype: &'a mut Archetype,
    i: usize,
    location: usize,
}

impl<'a> RemoveIterator<'a> {
    fn new(archetype: &'a mut Archetype, location: usize) -> Self {
        Self {
            archetype,
            i: 0,
            location,
        }
    }

    ///return which entity is being moved, and where it will end up
    pub fn moved_entity(&self) -> Option<(Entity, usize)> {
        self.archetype.entities.last().map(|e| (*e, self.location))
    }
}

impl<'a> Iterator for RemoveIterator<'a> {
    type Item = (Entity, DropLocation<'a>);

    fn next(&mut self) -> Option<Self::Item> {
        if self.i >= self.archetype.columns.len() {
            return None;
        }

        unsafe {
            let location = self.archetype.columns[self.i].offset(self.location);
            Some((
                self.archetype.components[self.i],
                DropLocation::at(location),
            ))
        }
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
