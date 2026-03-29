mod archetype;
mod archetype_manager;
mod component_bridge;
mod entity_manager;
mod merge_iter;
mod query;
mod query_manager;

use crate::registry::archetype_manager::ArchetypeManager;
use crate::registry::entity_manager::{EntityLocation, EntityManager};
use crate::registry::query_manager::QueryManager;
use crate::shared::id::{Component, ComponentDescriptor, ComponentIdentity, Entity};
use merge_iter::MergeIter;
use reflexion::erased::{DropLocation, ErasedRef};
use std::iter;
use std::iter::zip;

pub type ArchetypeIndex = usize;
pub type EntityIndex = usize;
pub type ColumnIndex = usize;
pub type QueryIndex = usize;
pub(crate) struct MovedEntity {
    entity: Entity,
    new_location: EntityLocation,
}

pub struct Registry {
    // where each entity is located in the registry
    entities: EntityManager,
    archetypes: ArchetypeManager,
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

    pub fn create_empty_entity(&mut self) -> Entity {
        self.entities.allocate(|entity| {
            self.archetypes.push(
                ArchetypeManager::NO_COMPONENT_ARCHETYPE,
                entity,
                iter::empty(),
            )
        })
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

    //TODO: Remove the genric used here and make that iterator ABI-safe
    pub fn create_entity<'a>(
        &mut self,
        components: &[Component],
        values: impl ExactSizeIterator<Item = DropLocation<'a>>,
    ) -> Entity {
        assert!(
            components
                .windows(2)
                .all(|slice| { if let [a, b] = slice { a != b } else { false } }),
            "components must be different"
        );

        let archetype_index = self.archetypes.find_or_create_archetype(components.into());
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
        //todo add proper error handling
        assert!(
            components
                .windows(2)
                .all(|slice| { if let [a, b] = slice { a != b } else { false } }),
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

        let dst_archetype_index = self.archetypes.find_or_create_archetype(dst_header);

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

    /*pub fn build_query(&self, builder: QueryBuilder) -> Query {
        let QueryBuilder {
            requested_components,
        } = builder;

        let accessible_components = HashMap::from_iter(requested_components
            .iter()
            .cloned()
            .filter(|component| self.component_bridge.is_sized_component(component))
            .flat_map(|component| self.component_bridge.find_identity(&component))
            .enumerate()
            .map(|(a, b)| (b, a))
        );

        if requested_components.len() == 0 {
            return Query {
                accessible_components,
                archetypes: (0..self.archetypes.len()).map(|u| (u, vec![])).collect(),
            };
        };

        // the algorithm is simple for now, we compute the intersection between the archetype map of all components
        let mut building: Vec<ColumnIndex> = Vec::with_capacity(accessible_components.len());
        let archetypes: Vec<_> = self
            .component_location
            .get(&requested_components[0])
            .into_iter()
            .flat_map(HashMap::iter)
            .flat_map(|(archetype_index, &column_index)| {
                building.clear();
                if self
                    .component_bridge
                    .is_sized_component(&requested_components[0])
                {
                    building.push(column_index);
                }
                for other_component in &requested_components[1..] {
                    let map = self.component_location.get(other_component)?;
                    let pos = map.get(archetype_index)?;
                    if self.component_bridge.is_sized_component(other_component) {
                        building.push(*pos);
                    }
                }
                Some((*archetype_index, building.clone()))
            })
            .collect();

        Query {
            accessible_components,
            archetypes,
        }
    }*/
}
