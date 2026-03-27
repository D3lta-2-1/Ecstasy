use crate::shared::id::{Component, ComponentDescriptor, ComponentIdentity, Entity};
use reflexion::typeinfo::{TypeInfo, TypeInfoImpl};
use std::collections::HashMap;

#[derive(Default)]
pub struct ComponentIdentityBridge {
    component_to_type_info: HashMap<Entity, ComponentDescriptor>,
    type_info_from_component: HashMap<ComponentIdentity, Entity>,
}

impl ComponentIdentityBridge {
    pub fn add(&mut self, component_descriptor: ComponentDescriptor, entity: Entity) {
        self.component_to_type_info
            .insert(entity, component_descriptor);
        self.type_info_from_component
            .insert(component_descriptor.identity, entity);
    }

    pub fn find_type_info(&self, component: &Component) -> TypeInfo {
        self.component_to_type_info
            .get(component)
            .map(|descriptor| descriptor.type_info)
            .unwrap_or(TypeInfoImpl::EMPTY)
    }

    pub fn find_identity(&self, component: &Component) -> Option<ComponentIdentity> {
        self.component_to_type_info
            .get(component)
            .map(|descriptor| descriptor.identity)
    }

    pub fn is_sized_component(&self, component: &Component) -> bool {
        self.find_type_info(component).layout.size > 0
    }

    pub fn find_component(&self, component_identity: &ComponentIdentity) -> Option<Component> {
        self.type_info_from_component
            .get(component_identity)
            .cloned()
    }
}
