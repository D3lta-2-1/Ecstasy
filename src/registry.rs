use crate::archetype::Archetype;
use crate::component_bridge::ComponentIdentityBridge;
use crate::merge_iter::MergeIter;
use crate::shared::id::{Component, ComponentIdentity, Entity};
use reflexion::erased::{DropLocation, ErasedRef};
use std::collections::HashMap;
use std::fmt::{Debug, Formatter};
use std::iter;
use std::iter::zip;

pub type ArchetypeIndex = usize;
pub type EntityIndex = usize;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct EntityLocation {
    archetype_index: ArchetypeIndex,
    entity_index: EntityIndex,
}

pub struct Registry {
    // used to generate unique entity ids
    entity_counter: u32, // TODO: introduce it recycling mechanism to avoid overflow
    // where each entity is located in the registry
    entities: HashMap<Entity, EntityLocation>, // TODO: update that to a sparse set
    // store the archetypes, each archetype is a collection of entities with the same components
    archetypes: Vec<Archetype>, // TODO: promote that as an abstract storage
    // allow us to find the archetype based on what components it contains
    components_set_to_archetype: HashMap<Vec<Component>, ArchetypeIndex>,
    // where components are located, used for :
    // - single component query with (component, archetypeID) query
    // - matching archetype query using the second hash map as a set
    component_location: HashMap<Component, HashMap<ArchetypeIndex, usize>>,
    // mapping between component type and id
    component_bridge: ComponentIdentityBridge,
}

impl Debug for Registry {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "{} archetype stored", self.archetypes.len())?;
        for archetype in &self.archetypes {
            archetype.fmt(f)?;
        }
        Ok(())
    }
}

/// Design choices
/// - components are entities, so you can add components to the components.
/// - archetype columns are order to easy component move between them
/// - For now, entities stay anonymous, we don't store their names or paths.
/// all function exposed by the registry should be ABI safe. It's not the case now, but that mean NO GENERIC can be used here
impl Registry {
    const NO_COMPONENT_ARCHETYPE: ArchetypeIndex = 0;

    pub fn new() -> Self {
        let component_bridge = ComponentIdentityBridge::default();
        let components_set_to_archetype = HashMap::from([
            (vec![], Self::NO_COMPONENT_ARCHETYPE), // no component archetype
        ]);

        let archetypes = vec![
            Archetype::new(vec![], &component_bridge), // no component archetype
        ];

        Registry {
            entity_counter: 0,
            entities: HashMap::new(),
            archetypes,
            components_set_to_archetype,
            component_location: HashMap::new(),
            component_bridge,
        }
    }

    fn new_entity_id(&mut self) -> Entity {
        let entity = Entity(self.entity_counter);
        self.entity_counter += 1;
        entity
    }

    pub fn find_component(&self, component: &ComponentIdentity) -> Option<Entity> {
        self.component_bridge.find_component(component)
    }

    pub fn find_or_register_component(&mut self, component: &ComponentIdentity) -> Entity {
        if let Some(e) = self.component_bridge.find_component(component) {
            e
        } else {
            let e = self.create_empty_entity();
            self.component_bridge.add(*component, e);
            e
        }
    }

    /// try to find an archetype that contains the components, if not found, create a new archetype with the components.
    fn find_or_create_archetype(&mut self, components: Vec<Component>) -> ArchetypeIndex {
        debug_assert!(components.is_sorted());
        let archetype_index = self.components_set_to_archetype.get(&components);
        if let Some(index) = archetype_index {
            return *index;
        }
        let new_archetype = Archetype::new(components.clone(), &self.component_bridge);
        let archetype_index = self.archetypes.len();
        for (i, component) in components.iter().enumerate() {
            self.component_location
                .entry(*component)
                .or_insert(HashMap::new())
                .insert(archetype_index, i);
        }

        self.archetypes.push(new_archetype);
        self.components_set_to_archetype
            .insert(components, archetype_index);
        archetype_index
    }

    fn update_location(
        // TODO: remove that when proper Entity location map will be added
        entities: &mut HashMap<Entity, EntityLocation>,
        entity: Entity,
        archetype_index: ArchetypeIndex,
        entity_index: EntityIndex,
    ) {
        entities.insert(
            entity,
            EntityLocation {
                archetype_index,
                entity_index,
            },
        );
    }

    pub fn create_empty_entity(&mut self) -> Entity {
        let entity = self.new_entity_id();
        let entity_index = self.archetypes[Self::NO_COMPONENT_ARCHETYPE]
            .push(entity, iter::empty())
            .expect("insertion failed");
        Self::update_location(
            &mut self.entities,
            entity,
            Self::NO_COMPONENT_ARCHETYPE,
            entity_index,
        );
        entity
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

        let entity = self.new_entity_id();
        let archetype = self.find_or_create_archetype(components.into());
        let entity_index = self.archetypes[archetype]
            .push(entity, zip(components.iter().cloned(), values))
            .expect("insertion failed");
        Self::update_location(&mut self.entities, entity, archetype, entity_index);
        entity
    }

    pub fn add_components<'s: 'a, 'a>(
        &'s mut self,
        entity: Entity,
        components: &[Component],
        values: impl ExactSizeIterator<Item = DropLocation<'a>>,
    ) -> Option<()> {
        assert!(
            components
                .windows(2)
                .all(|slice| { if let [a, b] = slice { a != b } else { false } }),
            "components must be different"
        );
        assert!(components.len() > 0);

        let EntityLocation {
            archetype_index: src_archetype_index,
            entity_index,
        } = *self.entities.get(&entity)?;
        let base_component = self.archetypes[src_archetype_index].get_descriptor();
        let dst_header: Vec<_> = MergeIter::new(base_component, components)
            .cloned()
            .collect();

        let dst_archetype_index = self.find_or_create_archetype(dst_header);

        if src_archetype_index != dst_archetype_index {
            self.move_entity(entity, entity_index, src_archetype_index, dst_archetype_index, components, values)
        } else {
            let archetype = &mut self.archetypes[dst_archetype_index];
            for (component, value) in zip(components, values) {
                // the archetype already exist, because the entity is already in, so both of these operations are safe
                let column = self.component_location.get(component).unwrap().get(&dst_archetype_index).unwrap();
                archetype.mut_at(*column, entity_index).write(value);
            }
            Some(())
        }
    }

    fn move_entity<'s: 'a, 'a>(
        &'s mut self,
        entity: Entity,
        entity_index: usize,
        src_archetype_index: usize,
        dst_archetype_index: usize,
        components: &[Component],
        values: impl ExactSizeIterator<Item = DropLocation<'a>>,
    ) -> Option<()> {
        let [src_archetype, dst_archetype] = self
            .archetypes
            .get_disjoint_mut([src_archetype_index, dst_archetype_index])
            .unwrap(); //TODO: add support for component overwrite only

        let actual_values = src_archetype.swap_remove(entity_index);
        let new_value_iter = zip(components.iter().cloned(), values);
        let moved_entity = actual_values.moved_entity();

        let values =
            MergeIter::with_custom_ordering(new_value_iter, actual_values, |(c1, _), (c2, _)| {
                c1.cmp(c2)
            });

        let new_location = dst_archetype
            .push(entity, values.into_iter())
            .expect("insertion failed");

        Self::update_location(
            &mut self.entities,
            entity,
            dst_archetype_index,
            new_location,
        );
        if let Some((moved_entity, new_location)) = moved_entity {
            Self::update_location(
                &mut self.entities,
                moved_entity,
                src_archetype_index,
                new_location,
            );
        }
        Some(())
    }

pub fn get_one_component<'a>(
        &'a self,
        entity: Entity,
        component: Component,
    ) -> Option<ErasedRef<'a>> {
        let EntityLocation {
            archetype_index,
            entity_index,
        } = self.entities.get(&entity)?.clone();
        let map = self.component_location.get(&component)?;
        let column = map.get(&archetype_index)?.clone();
        Some(self.archetypes[archetype_index].ref_at(column, entity_index))
    }
}
