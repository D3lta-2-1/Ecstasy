use std::collections::HashMap;
use std::iter;
use crate::archetype::Archetype;
use crate::shared::id::{ComponentIdentity, Entity, RuntimeComponentIdentity};
use crate::shared::reflexion::{SourceData, TypeInfo, TypeInfoProvider};
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
    components_set_to_archetype: HashMap<Vec<Entity>, ArchetypeIndex>,
    // each component is represented by a single entity, we can find this entity from its identity
    component_finder: HashMap<ComponentIdentity, Entity>,
}


/// Design choices
/// - components are entities, so you can add components to the components.
/// - columns are not order during archetype creation, they are likely to be in the same order as the components in the archetype descriptor, but this will not be guaranteed.
/// - For now, entities stay anonymous, we don't store their names or paths.


impl Registry {

    const NO_COMPONENT_ARCHETYPE: ArchetypeIndex = 0;
    const ONLY_COMPONENT_ARCHETYPE: ArchetypeIndex = 1;
    const COMPONENT_META_DATA_COMPONENT: Entity = Entity(0);

    pub fn new() -> Self {
        let components_set_to_archetype = HashMap::from([
            (vec![], Self::NO_COMPONENT_ARCHETYPE), // no component archetype
            (vec![Self::COMPONENT_META_DATA_COMPONENT], Self::ONLY_COMPONENT_ARCHETYPE), // only component archetype
        ]);
        let mut entities = HashMap::new();

        let mut archetypes = vec![
            Archetype::new(iter::empty()), // no component archetype
            Archetype::new([RuntimeComponentIdentity{
                id: Self::COMPONENT_META_DATA_COMPONENT,
                type_info: ComponentIdentity::TYPE_INFO,
            }].into()), // only component archetype
        ];

        let component_identity = ComponentIdentity {
            path: "ecstasy",
            name: "ComponentIdentity",
            type_info: ComponentIdentity::TYPE_INFO,
        };
        SourceData::new(component_identity, Self::COMPONENT_META_DATA_COMPONENT, |components| {
            let location = archetypes[Self::ONLY_COMPONENT_ARCHETYPE].add_entity(Self::COMPONENT_META_DATA_COMPONENT, components.iter_mut());
            entities.insert(Self::COMPONENT_META_DATA_COMPONENT, EntityLocation {
                archetype_index: Self::ONLY_COMPONENT_ARCHETYPE,
                entity_index: location,
            });
        });

        let component_finder = HashMap::from([(component_identity, Self::COMPONENT_META_DATA_COMPONENT)]);

        Registry {
            entity_counter: 1,
            entities,
            archetypes,
            components_set_to_archetype,
            component_finder,
        }
    }

    pub fn find_component(&self, component: &ComponentIdentity) -> Option<Entity> {
        self.component_finder.get(component).copied()
    }

    pub fn create_empty_entity(&mut self) -> Entity {
        let entity = Entity(self.entity_counter);
        self.entity_counter += 1;
        let archetype = &mut self.archetypes[Self::NO_COMPONENT_ARCHETYPE];
        let entity_index = archetype.add_entity(entity, iter::empty());
        self.entities.insert(entity, EntityLocation {
            archetype_index: Self::NO_COMPONENT_ARCHETYPE,
            entity_index,
        });
        entity
    }

    /// try to find an archetype that contains the components, if not found, create a new archetype with the components.
    fn find_or_create_archetype(&mut self, components: &Vec<Entity>) -> ArchetypeIndex {
        let archetype_index = self.components_set_to_archetype.get(components);
        if let Some(index) = archetype_index {
            return *index;
        }
        let iterator = components.iter().map(|&id| {
            let EntityLocation { archetype_index, entity_index } = *self.entities.get(&id).expect("Component not found in registry");
            let type_info = self.archetypes[archetype_index].get_component(entity_index, id);
            let type_info = unsafe {
                type_info.map(|ptr| ptr.as_ref::<ComponentIdentity>().type_info).unwrap_or(TypeInfo::EMPTY)
            };
            RuntimeComponentIdentity {
                id,
                type_info,
            }
        });
        let new_archetype = Archetype::new(iterator);
        let archetype_index = self.archetypes.len();
        self.archetypes.push(new_archetype);
        self.components_set_to_archetype.insert(components.clone(), archetype_index);
        archetype_index
    }

    pub fn add_component_to_entity(&mut self, entity: Entity, components: &mut [SourceData]) {
        let EntityLocation{
            archetype_index,
            entity_index,
        } = *self.entities.get(&entity).expect("Entity not found in registry");
        let mut components_set = self.archetypes[archetype_index].get_descriptor().clone();
        components_set.extend(components.iter().map(|c| c.get_component_id()));
        components_set.sort();
        let target_archetype_index = self.find_or_create_archetype(&components_set);

        // little trick to borrow two values at the same time, have to optimize that later
        // TODO: support that case in the future
        let [src, dst] = self.archetypes.get_disjoint_mut([archetype_index, target_archetype_index]).expect("Cannot add component to an entity that already has this component");
        let (moved_entity, moved_location) = src.remove_entity(entity_index, |stream| {

        });


    }
}