use std::any::TypeId;
use std::collections::HashMap;
use std::hash::{Hash};
use crate::registry::Registry;
use crate::shared::id::Entity;

/// the registry header is the final interface between the ECS internal and "external" world.
/// it's where all clean generic methods are defined
/// each binary accessing the ECS across DLL boundaries will get a copy of all this code and data structure
/// it's the perfect place to some target local caching such as type_id <-> component identity

pub trait Component {
    const PATH: &'static str;
    const NAME: &'static str;
}

pub struct RegistryHeader {
    registry: Registry,
    component_cache: HashMap<TypeId, Entity>,
}

impl RegistryHeader {
    /*fn add_component<T: Component>(&mut self, entity: Entity, component: T) {
        let type_id = TypeId::of::<T>();
        if !self.component_cache.contains_key(&type_id) {
            self.component_cache.insert(type_id, entity);
        }
        self.registry.add_component(entity, component, value);
    }*/
}