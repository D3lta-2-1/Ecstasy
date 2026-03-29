use crate::registry::archetype::Archetype;
use crate::registry::component_bridge::ComponentIdentityBridge;
use crate::registry::entity_manager::EntityLocation;
use crate::registry::merge_iter::MergeIter;
use crate::registry::{ArchetypeIndex, ColumnIndex, MovedEntity};
use crate::shared::id::{Component, ComponentDescriptor, ComponentIdentity, Entity};
use reflexion::erased::{DropLocation, ErasedRef};
use std::collections::HashMap;
use std::iter::zip;

// releasing archetype can be very challenging, for now, they nerver get released
pub struct ArchetypeManager {
    // store the archetypes, each archetype is a collection of entities with the same components
    archetypes: Vec<Archetype>, // TODO: promote that as an abstract storage
    // allow us to find the archetype based on what components it contains
    components_set_to_archetype: HashMap<Vec<Component>, ArchetypeIndex>,
    // where components are located, used for :
    // - single component query with (component, archetypeID) query
    // - matching archetype query using the second hash map as a set
    component_location: HashMap<Component, HashMap<ArchetypeIndex, ColumnIndex>>,
    // mapping between component type and id
    component_bridge: ComponentIdentityBridge,
}

impl ArchetypeManager {
    pub const NO_COMPONENT_ARCHETYPE: ArchetypeIndex = 0;

    pub fn new() -> Self {
        let component_bridge = ComponentIdentityBridge::default();
        let components_set_to_archetype = HashMap::from([
            (vec![], Self::NO_COMPONENT_ARCHETYPE), // no component archetype
        ]);

        let archetypes = vec![
            Archetype::new(vec![], &component_bridge), // no component archetype
        ];
        Self {
            archetypes,
            components_set_to_archetype,
            component_location: Default::default(),
            component_bridge,
        }
    }

    /// create a new link between a ``ComponentDescriptor`` and an ``Entity``
    pub fn add_new_component_mapping(&mut self, component: ComponentDescriptor, entity: Entity) {
        self.component_bridge.add(component, entity);
    }

    /// a getter to get the corresponding Archetype
    pub fn get_archetype(&self, index: ArchetypeIndex) -> &Archetype {
        &self.archetypes[index]
    }

    /// push an entity at a new location, the iterator needs to be sorted
    pub fn push<'a>(
        &mut self,
        archetype_index: ArchetypeIndex,
        entity: Entity,
        components: impl Iterator<Item = (Component, DropLocation<'a>)>,
    ) -> EntityLocation {
        let entity_index = self.archetypes[archetype_index]
            .push(entity, components)
            .expect("insertion failed");
        EntityLocation {
            archetype_index,
            entity_index,
        }
    }

    pub fn get_component_at(
        &'_ self,
        EntityLocation {
            archetype_index,
            entity_index,
        }: EntityLocation,
        component: ComponentIdentity,
    ) -> Option<ErasedRef<'_>> {
        let component = self.component_bridge.find_component(&component)?;
        let map = self.component_location.get(&component)?;
        let column = map.get(&archetype_index)?.clone();
        Some(self.archetypes[archetype_index].ref_at(column, entity_index))
    }

    /// write an iterator at a given location, the archetype must already have an initialized component
    /// the iterator doesn't need to be sorted
    pub fn set_components<'a>(
        &mut self,
        EntityLocation {
            archetype_index,
            entity_index,
        }: EntityLocation,
        components: impl Iterator<Item = (Component, DropLocation<'a>)>,
    ) {
        let archetype = &mut self.archetypes[archetype_index];
        for (component, value) in components {
            // the archetype already exist, because the entity is already in, so both of these operations are safe
            let column = self
                .component_location
                .get(&component)
                .expect("no location associated with this component")
                .get(&archetype_index)
                .expect("archetype not found");
            archetype.mut_at(*column, entity_index).write(value);
        }
    }

    /// try to find an archetype that contains the components, if not found, create a new archetype with the components.
    pub fn find_or_create_archetype(&mut self, components: Vec<Component>) -> ArchetypeIndex {
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

    pub fn find_component(&self, component: &ComponentIdentity) -> Option<Entity> {
        self.component_bridge.find_component(component)
    }

    pub fn find_column(
        &self,
        component: Component,
        archetype_index: ArchetypeIndex,
    ) -> ColumnIndex {
        *self
            .component_location
            .get(&component)
            .expect("no location associated with this component")
            .get(&archetype_index)
            .expect("archetype not found")
    }

    /// move an entity from an archetype to another,
    /// if the additional components only overwrite the old one (if there isn't any move)
    /// it will panic
    pub fn move_entity<'s: 'a, 'a>(
        &'s mut self,
        entity: Entity,
        src_location: EntityLocation,
        dst_archetype_index: ArchetypeIndex,
        components: &[Component],
        values: impl ExactSizeIterator<Item = DropLocation<'a>>,
    ) -> (MovedEntity, Option<MovedEntity>) {
        let [src_archetype, dst_archetype] = self
            .archetypes
            .get_disjoint_mut([src_location.archetype_index, dst_archetype_index])
            .expect("src and target archetype are the same");

        let actual_values = src_archetype.swap_remove(src_location.entity_index);
        let new_value_iter = zip(components.iter().cloned(), values);
        let moved_entity = actual_values.moved_entity();

        let values =
            MergeIter::with_custom_ordering(new_value_iter, actual_values, |(c1, _), (c2, _)| {
                c1.cmp(c2)
            });

        let new_location = dst_archetype
            .push(entity, values.into_iter())
            .expect("insertion failed");

        let mov1 = MovedEntity {
            entity,
            new_location: EntityLocation {
                archetype_index: dst_archetype_index,
                entity_index: new_location,
            },
        };
        let mov2 = moved_entity.map(|(entity, new_location)| MovedEntity {
            entity,
            new_location: EntityLocation {
                archetype_index: dst_archetype_index,
                entity_index: new_location,
            },
        });
        (mov1, mov2)
    }
}
