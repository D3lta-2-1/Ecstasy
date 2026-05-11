mod archetype;
mod archetype_manager;
mod component_bridge;
mod entity_manager;
mod merge_iter;
mod query;
mod query_manager;

use archetype_manager::ArchetypeManager;
use entity_manager::EntityManager;
use merge_iter::MergeIter;
use query_manager::QueryManager;
use reflexion::erased::{ErasedMutPointer, ErasedRef};

pub use registry_ffi::{
    ArchetypeIndex, ColumnIndex, EntityIndex, EntityLocation, LocalColumnIndex, QueryIndex,
};
use registry_ffi::{Component, ComponentDescriptor, ComponentIdentity, Entity};
use std::{iter, iter::zip};

use reflexion::drop_location::DropLocation;

pub(crate) struct MovedEntity {
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
            archetypes: ArchetypeManager::new(),
            queries: QueryManager::default(),
        }
    }

    pub fn find_or_register_component(&mut self, component: &ComponentDescriptor) -> Entity {
        if let Some(e) = self.archetypes.find_component(&component.identity) {
            e
        } else {
            let e = self.create_empty_permanent_entity();
            self.archetypes.add_new_component_mapping(*component, e);
            e
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

    //TODO: Remove the genric used here and make that iterator ABI-safe
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

    pub fn add_components<'s: 'a, 'a>(
        &'s mut self,
        entity: Entity,
        components: &[Component],
        values: impl ExactSizeIterator<Item = DropLocation<'a>>,
    ) -> Result<(), ()> {
        //TODO: add proper error handling
        assert!(
            components
                .windows(2)
                .all(|slice| { if let [a, b] = slice { a < b } else { false } }),
            "components must be different"
        );
        assert!(components.len() > 0);

        let src_location = self.entities.get(entity).ok_or(())?;
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
        identity: ComponentIdentity,
    ) -> Option<ErasedRef<'_>> {
        let loc = self.entities.get(entity)?;
        self.archetypes.get_component_at(loc, identity)
    }

    pub fn location(&self, entity: Entity) -> Option<EntityLocation> {
        self.entities.get(entity)
    }

    /// this function will return the query ID associated with this builder, and create if required
    /// Queries can't be deleted, they are meant to be used through systems
    pub fn get_query_id(&mut self, builder: &[Component]) -> QueryIndex {
        self.queries.insert_query(builder.to_vec(), |builder| {
            self.archetypes.create_query(builder)
        })
    }

    pub fn query_get_local_column_index(
        &self,
        query_index: QueryIndex,
        identity: &ComponentIdentity,
    ) -> LocalColumnIndex {
        let query = self.queries.get_query(query_index);
        *query
            .accessible_components
            .get(identity)
            .expect("this query doesn't contain this component")
    }

    pub fn query_get_columns_index(
        &self,
        query_index: QueryIndex,
        archetype_index: ArchetypeIndex,
    ) -> Option<&[ColumnIndex]> {
        Some(
            &self
                .queries
                .get_query(query_index)
                .archetypes
                .get(&archetype_index)?,
        )
    }

    //TODO: mutability here is really unclear, this function is used in query, where it's forbidden to add/delete, but components can be mutated
    pub unsafe fn get_colum_begin(
        &self,
        archetype_index: ArchetypeIndex,
        columns: &[ColumnIndex],
        starts: &mut [ErasedMutPointer],
    ) -> &[Entity] {
        unsafe {
            self.archetypes
                .get_colum_begin(archetype_index, columns, starts)
        }
    }
}

use reflexion::{
    ffi_collection::FfiCollectionIter,
    ffi_enum::{FfiOption, FfiResult},
    ffi_slice::FfiSlice,
};

impl registry_ffi::Registry for Registry {
    extern "C" fn find_or_register_component(
        &mut self,
        component: &ComponentDescriptor,
    ) -> Component {
        self.find_or_register_component(component)
    }

    extern "C" fn create_empty_entity(&mut self) -> Entity {
        self.create_empty_entity()
    }

    extern "C" fn create_entity<'a>(
        &mut self,
        components: FfiSlice<&Component>,
        values: FfiCollectionIter<DropLocation<'a>>,
    ) -> Entity {
        self.create_entity(components.into(), values)
    }

    extern "C" fn add_components<'s: 'a, 'a>(
        &'s mut self,
        entity: Entity,
        components: FfiSlice<&Component>,
        values: FfiCollectionIter<DropLocation<'a>>,
    ) -> FfiResult<(), ()> {
        self.add_components(entity, components.into(), values)
            .into()
    }

    extern "C" fn get_one_component(
        &self,
        entity: Entity,
        identity: ComponentIdentity,
    ) -> FfiOption<ErasedRef<'_>> {
        self.get_one_component(entity, identity).into()
    }

    extern "C" fn location(&self, entity: Entity) -> FfiOption<EntityLocation> {
        self.location(entity).into()
    }

    extern "C" fn get_query_id(&mut self, builder: FfiSlice<&Component>) -> QueryIndex {
        self.get_query_id(builder.into()).into()
    }

    extern "C" fn query_get_local_column_index(
        &self,
        query_index: QueryIndex,
        identity: &ComponentIdentity,
    ) -> LocalColumnIndex {
        self.query_get_local_column_index(query_index, identity)
    }

    extern "C" fn query_get_columns_index(
        &self,
        query_index: QueryIndex,
        archetype_index: ArchetypeIndex,
    ) -> FfiOption<FfiSlice<&ColumnIndex>> {
        let value: Option<FfiSlice<&ColumnIndex>> = self
            .query_get_columns_index(query_index, archetype_index)
            .map(|v| v.into());
        value.into()
    }

    unsafe extern "C" fn get_colum_begin<'a>(
        &'a self,
        archetype_index: ArchetypeIndex,
        columns: FfiSlice<&ColumnIndex>,
        starts: FfiSlice<&mut ErasedMutPointer>,
    ) -> FfiSlice<&'a Entity> {
        unsafe {
            self.get_colum_begin(archetype_index, columns.into(), starts.into())
                .into()
        }
    }
}
