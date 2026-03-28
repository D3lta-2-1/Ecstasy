use reflexion::typeinfo::TypeInfo;
use std::fmt::{Debug, Formatter};
use std::num::NonZeroU32;

/// A unique identifier for an entity in the ECS. Entities can both be a component or the concept the link entities together.
/// an entity is nothing more than a disguised integer.
/// Option<Entity> and both entity are guaranty to be the same size, in FFI, 0 is guaranteed to be None
/// The internal layout of an entity is <generation, 8 bit> | <identifier, 24 bits>
/// - Generation 0 is never used, and reserved for futur uses.
/// - Generation 1 is for long living entities such a sized components.
/// - The remaining generation 2..255 are used for "short living entities"
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(transparent)]
pub struct Entity(NonZeroU32);

impl Entity {
    pub fn new(generation: u8, index: u32) -> Self {
        assert!(generation > 0, "generation 0 is reserved");
        assert_eq!(index & 0xff000000, 0, "index upper limit reached");
        let generation = generation as u32;
        let value = index | (generation << 24);
        Self(NonZeroU32::new(value).unwrap())
    }

    pub fn generation(self) -> u8 {
        (self.0.get() >> 24) as u8
    }

    pub fn index(self) -> u32 {
        self.0.get() & 0xffffff
    }
}

#[test]
fn test_entity() {
    let generation = 38;
    let index = 144;
    let entity = Entity::new(generation, index);
    assert_eq!(generation, entity.generation());
    assert_eq!(index, entity.index());
}

impl Debug for Entity {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Entity(generation: {}, id: {})",
            self.generation(),
            self.index()
        )
    }
}

/// an alias for readability when an entity is used as a component
pub type Component = Entity;

/// a fully qualified component identity, used to get ComponentData from a component path and name.
/// it also checks that the layout matches to avoid type mismatches.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ComponentIdentity {
    pub path: &'static str, //consider to upgrade for a Cow
    pub name: &'static str,
}

impl ComponentIdentity {
    pub const EMPTY: Self = Self {
        path: "ecstasy",
        name: "empty",
    };
}

#[derive(Copy, Clone, Debug)]
pub struct ComponentDescriptor {
    pub identity: ComponentIdentity,
    pub type_info: TypeInfo,
}
