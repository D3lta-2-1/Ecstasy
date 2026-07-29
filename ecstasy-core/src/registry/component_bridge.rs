use ecstasy_ffi::{Component, ComponentDescriptor, Entity, TypeIdentity};
use reflexion::typeinfo::{TypeInfo, TypeInfoImpl};
use std::collections::HashMap;

#[derive(Default)]
pub struct ComponentIdentityBridge {
    component_to_type_info: HashMap<Entity, ComponentDescriptor>,
    type_info_from_component: HashMap<TypeIdentity, Entity>,
}

pub struct ComponentInfo {
    pub type_info: TypeInfo,
    pub versioned: bool,
}

impl ComponentIdentityBridge {
    pub fn add(&mut self, component_descriptor: ComponentDescriptor, entity: Entity) {
        self.component_to_type_info
            .insert(entity, component_descriptor);
        self.type_info_from_component
            .insert(component_descriptor.identity, entity);
    }

    pub fn find_type_info(&self, component: &Component) -> ComponentInfo {
        self.component_to_type_info
            .get(component)
            .map(|descriptor| ComponentInfo {
                type_info: descriptor.type_info,
                versioned: descriptor.versioned,
            })
            .unwrap_or(ComponentInfo {
                type_info: TypeInfoImpl::EMPTY,
                versioned: false,
            })
    }

    pub fn find_identity(&self, component: &Component) -> Option<TypeIdentity> {
        self.component_to_type_info
            .get(component)
            .map(|descriptor| descriptor.identity)
    }

    pub fn is_sized_component(&self, component: &Component) -> bool {
        self.find_type_info(component)
            .type_info
            .unwrap()
            .layout
            .size
            > 0
    }

    pub fn find_component(&self, component_identity: &TypeIdentity) -> Option<Component> {
        self.type_info_from_component
            .get(component_identity)
            .cloned()
    }
}
