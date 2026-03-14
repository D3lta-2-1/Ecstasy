use crate::shared::id::{ComponentIdentity, Entity};
use std::collections::HashMap;

#[derive(Default)]
pub struct ComponentIdentityBridge {
    component_to_type_info: HashMap<Entity, ComponentIdentity>,
    type_info_from_component: HashMap<ComponentIdentity, Entity>,
}

impl ComponentIdentityBridge {
    pub fn add(&mut self, component_identity: ComponentIdentity, entity: Entity) {
        self.component_to_type_info
            .insert(entity, component_identity);
        self.type_info_from_component
            .insert(component_identity, entity);
    }

    pub fn find_type_info(&self, entity: &Entity) -> ComponentIdentity {
        self.component_to_type_info
            .get(entity)
            .cloned()
            .unwrap_or(ComponentIdentity::EMPTY)
    }

    pub fn find_component(&self, component_identity: &ComponentIdentity) -> Option<Entity> {
        self.type_info_from_component
            .get(component_identity)
            .cloned()
    }
}
