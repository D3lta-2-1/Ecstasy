use std::alloc::{handle_alloc_error, Layout};
use std::collections::HashMap;
use crate::registry::EntityIndex;
use crate::shared::id::{Entity, RuntimeComponentIdentity};
use crate::shared::reflexion::{ErasedPointer, SourceData};

/// structure in charge of storing data for a specific entity

pub struct Archetype {
    entities: Vec<Entity>,
    columns: Vec<ErasedPointer>,
    columns_descriptor: Vec<Entity>,
    components_to_columns: HashMap<Entity, usize> // maps component id to column index
}

impl Archetype {
    pub fn new(components: impl Iterator<Item = RuntimeComponentIdentity>) -> Self {
        let mut columns = Vec::new();
        let mut columns_descriptor = Vec::new();
        let mut components_to_columns = HashMap::new();

        for RuntimeComponentIdentity { id, type_info} in components {
            let type_info = type_info;
            columns.push(ErasedPointer::from_type_info(type_info));
            columns_descriptor.push(id);
            components_to_columns.insert(id, columns.len() - 1);
        }

        Archetype {
            entities: Vec::new(),
            columns,
            columns_descriptor,
            components_to_columns,
        }
    }

    /// Return all components ids stored in the archetype.
    pub fn get_descriptor(&self) -> &Vec<Entity> {
        &self.columns_descriptor
    }

    fn grow_columns(&mut self, new_size: usize) {
        unsafe {
            for columm in self.columns.iter_mut() {
                if columm.is_null() {
                   *columm = ErasedPointer::allocate(columm.type_info, new_size);
                } else {
                    *columm = columm.reallocate(new_size);
                }
                if columm.is_null() { handle_alloc_error(Layout::from_size_align(columm.type_info.layout.size *  new_size, columm.type_info.layout.align).unwrap()) }
            }
        }
    }

    /// Adds an entity to the archetype, if the archetype is full, it will grow the columns to accommodate the new entity.
    /// Returns the index of the entity in the archetype.
    /// you must provide at least one source data for each component in the archetype.
    /// if more are provided, the latest will overwrite the previous ones, which will be dropped.
    /// # Panics
    /// - Panics if the component id is not found in the archetype.
    /// - Panics if the source data is not provided for all components in the archetype.
    pub fn add_entity(&mut self, entity: Entity, source_data: impl Iterator<Item = &mut SourceData>) -> EntityIndex {
        let is_full = self.entities.capacity() == self.entities.len();
        let location = self.entities.len();
        self.entities.push(entity);
        if is_full {
            self.grow_columns(self.entities.capacity());
        }
        let mut initialised_columns = vec![false; self.columns.len()];
        for component in source_data {
            let column_index = self.components_to_columns.get(&component.get_component_id()).expect("Component not found in archetype");
            unsafe {
                let column = &mut self.columns[*column_index];
                let target = column.offset(location);
                if initialised_columns[*column_index] {
                    target.drop_in_place();
                } else {
                    initialised_columns[*column_index] = true;
                }
                component.write_to(target);
            }
        }
        assert!(initialised_columns.iter().all(|&x| x), "Not all components were initialised for the entity");
        location
    }

    /// Swap remove an entity at a given location in the archetype.
    /// Return the entity that was removed and the new location of the entity that was swapped in.
    pub fn remove_entity<T, Q>(&mut self, location: EntityIndex, removed_data_inserter: T) -> (Entity, EntityIndex)
        where
            T: FnOnce(Q),
            Q: IntoIterator<Item = SourceData>
    {
        // destroy or get rid of the data at the given location
        unsafe {
            let columns = self.columns.iter().map(|col| unsafe { col.offset(location) });
            let mut iter = self.columns_descriptor.iter().cloned().zip(columns).map(|(component_id, column)| SourceData::from_erased_pointer(component_id, column));
            removed_data_inserter(&mut iter);
            // Safety concern: does it drop the data correctly if the iterator isn't fully run?
            for _ in iter {};
        }

        // swap the last entity with the one at the given location
        self.entities.swap_remove(location);
        for column in &mut self.columns {
            unsafe {
                column.offset(location).copy_nonoverlapping_from(&column.offset(self.entities.len()));
            }
        }

        (self.entities[location], location)
    }

    /// Return a pointer to the component data for the entity at the given location.
    /// # Safety
    /// - The caller must ensure that the location is valid and within the bounds of the archetype.
    /// - The caller is responsible for the right usage of the returned pointer, that is the good mutability and the right type.
    pub fn get_component(&self, location: usize, component_id: Entity) -> Option<ErasedPointer> {
        let Some(column_index) = self.components_to_columns.get(&component_id).expect("Component not found in archetype");
        unsafe {
            let column = &self.columns[*column_index];
            column.offset(location)
        }
    }
}