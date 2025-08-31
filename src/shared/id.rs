use crate::shared::reflexion;

/// A unique identifier for an entity in the ECS. Entities can both be a component or the concept the link entities together.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Entity(pub(crate) u32);

/// a fully qualified component identity, used to get ComponentData from a component path and name.
/// it also checks that the layout matches to avoid type mismatches.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ComponentIdentity {
    pub path: &'static str,
    pub name: &'static str,
    pub type_info: &'static reflexion::TypeInfo,
}

/// Information about a known component in the ECS.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeComponentIdentity {
    pub id: Entity,
    pub type_info: &'static reflexion::TypeInfo,
}