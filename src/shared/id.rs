use reflexion::typeinfo::{TypeInfo, TypeInfoProvider};

/// A unique identifier for an entity in the ECS. Entities can both be a component or the concept the link entities together.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Entity(pub(crate) u32);

/// an alias for readability when an entity is used as a component
pub type Component = Entity;

/// a fully qualified component identity, used to get ComponentData from a component path and name.
/// it also checks that the layout matches to avoid type mismatches.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ComponentIdentity {
    pub path: &'static str, //consider to upgrade for a Cow
    pub name: &'static str,
    pub type_info: TypeInfo,
}

impl ComponentIdentity {
    pub const EMPTY: Self = Self {
        path: "ecstasy",
        name: "empty",
        type_info: <()>::TYPE_INFO,
    };
}
