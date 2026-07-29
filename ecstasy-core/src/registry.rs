mod archetype;
mod archetype_manager;
mod component_bridge;
mod entity_manager;

pub mod query;
mod query_manager;

use crate::merge_iter::MergeIter;
use archetype_manager::ArchetypeManager;
use entity_manager::EntityManager;
use query_manager::QueryManager;
use reflexion::erased::{ErasedMutPointer, ErasedRef};

use ecstasy_ffi::{
    self, Component, ComponentDescriptor, Entity, QueryBuilder, QuerySetOpaque, QuerySetVtableExt,
    RegistryError, TypeIdentity,
};
pub use ecstasy_ffi::{
    ArchetypeIndex, ColumnIndex, EntityIndex, EntityLocation, LocalColumnIndex, QuerySetIndex,
};
use std::{iter, iter::zip};

use reflexion::drop_location::DropLocation;

pub struct MovedEntity {
    entity: Entity,
    new_location: EntityLocation,
}

pub struct Registry {
    // where each entity is located in the registry
    entities: EntityManager,
    // archetype container
    archetypes: ArchetypeManager,
    // track, and maintain queries
    queries: QueryManager,
}

/// Design choices
/// - components are entities, so you can add components to the components.
/// - archetype columns are order to easy component move between them
/// - For now, entities stay anonymous, we don't store their names or paths.
/// all function exposed by the registry should be ABI safe. It's not the case now, but that mean NO GENERIC can be used here
impl Registry {
    pub fn new() -> Self {
        Registry {
            entities: EntityManager::default(),
            archetypes: ArchetypeManager::new(15), // TODO: make this configurable
            queries: QueryManager::default(),
        }
    }

    pub fn create_empty_permanent_entity(&mut self) -> Entity {
        self.entities.allocate_permanent(|entity| {
            self.archetypes.push(
                ArchetypeManager::NO_COMPONENT_ARCHETYPE,
                entity,
                iter::empty(),
            )
        })
    }

    pub fn create_empty_entity(&mut self) -> Entity {
        self.entities.allocate(|entity| {
            self.archetypes.push(
                ArchetypeManager::NO_COMPONENT_ARCHETYPE,
                entity,
                iter::empty(),
            )
        })
    }

    pub fn create_entity<'a>(
        &mut self,
        components: &[Component],
        values: impl ExactSizeIterator<Item = DropLocation<'a>>,
    ) -> Entity {
        assert!(
            components
                .windows(2)
                .all(|slice| { if let [a, b] = slice { a < b } else { false } }),
            "components must be different, and sorted"
        );

        let archetype_index = self
            .archetypes
            .find_or_create_archetype(components.into(), &mut self.queries);
        self.entities.allocate(|entity| {
            self.archetypes.push(
                archetype_index,
                entity,
                zip(components.iter().cloned(), values),
            )
        })
    }

    pub fn tick(&mut self) {
        self.archetypes.tick();
    }

    pub fn add_components<'s: 'a, 'a>(
        &'s mut self,
        entity: Entity,
        components: &[Component],
        values: impl ExactSizeIterator<Item = DropLocation<'a>>,
    ) -> Result<(), RegistryError> {
        assert!(
            components
                .windows(2)
                .all(|slice| { if let [a, b] = slice { a < b } else { false } }),
            "components must be different"
        );
        assert!(components.len() > 0);

        let src_location = self
            .entities
            .get(entity)
            .ok_or(RegistryError::EntityNotFound)?;
        let src_archetype_index = src_location.archetype_index;

        let base_component = self
            .archetypes
            .get_archetype(src_archetype_index)
            .get_descriptor();
        let dst_header: Vec<_> = MergeIter::new(base_component, components)
            .cloned()
            .collect();

        let dst_archetype_index = self
            .archetypes
            .find_or_create_archetype(dst_header, &mut self.queries);

        if src_archetype_index == dst_archetype_index {
            self.archetypes
                .set_components(src_location, zip(components.iter().cloned(), values));
            return Ok(());
        }

        let (mov1, mov2) = self.archetypes.move_entity(
            entity,
            src_location,
            dst_archetype_index,
            components,
            values,
        );
        self.entities.update_location(mov1);
        if let Some(mov2) = mov2 {
            self.entities.update_location(mov2)
        };
        Ok(())
    }

    pub fn get_one_component(
        &'_ self,
        entity: Entity,
        identity: TypeIdentity,
    ) -> Result<ErasedRef<'_>, RegistryError> {
        let loc = self
            .entities
            .get(entity)
            .ok_or(RegistryError::EntityNotFound)?;
        self.archetypes.get_component_at(loc, identity)
    }

    pub fn location(&self, entity: Entity) -> Result<EntityLocation, RegistryError> {
        self.entities
            .get(entity)
            .ok_or(RegistryError::EntityNotFound)
    }
}

use reflexion::{ffi_collection::FfiCollectionIter, ffi_enum::FfiResult, ffi_slice::FfiSlice};

impl ecstasy_ffi::Registry for Registry {
    extern "C-unwind" fn find_or_register_component(
        &mut self,
        component: &ComponentDescriptor,
    ) -> Component {
        if let Some(e) = self.archetypes.find_component(&component.identity) {
            e
        } else {
            let e = self.create_empty_permanent_entity();
            self.archetypes.add_new_component_mapping(*component, e);
            e
        }
    }

    extern "C-unwind" fn create_empty_entity(&mut self) -> Entity {
        self.create_empty_entity()
    }

    extern "C-unwind" fn create_entity<'a>(
        &mut self,
        components: FfiSlice<&Component>,
        values: FfiCollectionIter<DropLocation<'a>>,
    ) -> Entity {
        self.create_entity(components.into(), values)
    }

    extern "C-unwind" fn add_components<'s: 'a, 'a>(
        &'s mut self,
        entity: Entity,
        components: FfiSlice<&Component>,
        values: FfiCollectionIter<DropLocation<'a>>,
    ) -> FfiResult<(), RegistryError> {
        self.add_components(entity, components.into(), values)
            .into()
    }

    extern "C-unwind" fn get_one_component(
        &self,
        entity: Entity,
        identity: TypeIdentity,
    ) -> FfiResult<ErasedRef<'_>, RegistryError> {
        self.get_one_component(entity, identity).into()
    }

    extern "C-unwind" fn location(
        &self,
        entity: Entity,
    ) -> FfiResult<EntityLocation, RegistryError> {
        self.location(entity).into()
    }

    extern "C-unwind" fn get_query_id(&mut self, builder: QueryBuilder) -> QuerySetIndex {
        self.queries
            .get_query(builder, |builder| self.archetypes.create_query(builder))
    }

    extern "C-unwind" fn get_query(&self, id: QuerySetIndex) -> &QuerySetOpaque {
        self.queries.get_query_set(id).as_opaque()
    }

    //TODO: mutability here is really unclear, this function is used in query, where it's forbidden to add/delete, but components can be mutated
    // I'm considering adding atomis in the ECS to explicitly catch case where the same column is borrowed twice
    unsafe extern "C-unwind" fn get_column_begin<'a>(
        &'a self,
        archetype_index: ArchetypeIndex,
        columns: FfiSlice<&ColumnIndex>,
        starts: FfiSlice<&mut ErasedMutPointer>,
    ) -> FfiSlice<&'a Entity> {
        unsafe {
            self.archetypes
                .get_column_begin(archetype_index, columns.into(), starts.into())
                .into()
        }
    }

    extern "C-unwind" fn tick(&mut self) {
        self.tick();
    }
}
