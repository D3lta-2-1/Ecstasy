use reflexion::{
    drop_location::DropLocation,
    erased::{ErasedMutPointer, ErasedRef},
    ffi_collection::FfiCollectionIter,
    ffi_enum::{FfiOption, FfiResult},
    ffi_slice::FfiSlice,
    typeinfo::TypeInfo,
    vtable,
};
use std::{
    fmt::{Debug, Formatter},
    num::NonZeroU32,
};

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
/// unique identifier for an ``Archetype`` in the registry
pub type ArchetypeIndex = usize;
/// Index of a column in an ``Archetype``
pub type ColumnIndex = usize;
/// Position of an Entity its ``Archetype``
pub type EntityIndex = usize;
/// Postion of a column *inside* a ``Query``
pub type LocalColumnIndex = usize;
/// unique identifier for an ``Query`` in the registry
pub type QueryIndex = usize;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EntityLocation {
    pub archetype_index: ArchetypeIndex,
    pub entity_index: EntityIndex,
}

/// a fully qualified component identity, used to get ComponentData from a component path and name.
/// it also checks that the layout matches to avoid type mismatches.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ComponentIdentity {
    pub path: *const u8, //consider to upgrade for a Cow
    pub name: *const u8,
}

impl ComponentIdentity {
    pub const EMPTY: Self = Self {
        path: "ecstasy".as_ptr(),
        name: "empty".as_ptr(),
    };
}

#[derive(Copy, Clone, Debug)]
pub struct ComponentDescriptor {
    pub identity: ComponentIdentity,
    pub type_info: TypeInfo,
}

#[vtable]
pub trait Registry {
    /// Find the Entity that represent a given component
    extern "C" fn find_or_register_component(
        &mut self,
        component: &ComponentDescriptor,
    ) -> Component;
    /// Create a new Entity id, the new entity can be queried in empty queries.
    extern "C" fn create_empty_entity(&mut self) -> Entity;
    extern "C" fn create_entity<'a>(
        &mut self,
        components: FfiSlice<&Component>,
        values: FfiCollectionIter<DropLocation<'a>>,
    ) -> Entity;
    extern "C" fn add_components<'s: 'a, 'a>(
        &'s mut self,
        entity: Entity,
        components: FfiSlice<&Component>,
        values: FfiCollectionIter<DropLocation<'a>>,
    ) -> FfiResult<(), ()>;
    extern "C" fn get_one_component(
        &self,
        entity: Entity,
        identity: ComponentIdentity,
    ) -> FfiOption<ErasedRef<'_>>;
    extern "C" fn location(&self, entity: Entity) -> FfiOption<EntityLocation>;
    extern "C" fn get_query_id(&mut self, builder: FfiSlice<&Component>) -> QueryIndex;
    extern "C" fn query_get_local_column_index(
        &self,
        query_index: QueryIndex,
        identity: &ComponentIdentity,
    ) -> LocalColumnIndex;
    extern "C" fn query_get_columns_index(
        &self,
        query_index: QueryIndex,
        archetype_index: ArchetypeIndex,
    ) -> FfiOption<FfiSlice<&ColumnIndex>>;
    unsafe extern "C" fn get_colum_begin<'a>(
        &'a self,
        archetype_index: ArchetypeIndex,
        columns: FfiSlice<&ColumnIndex>,
        starts: FfiSlice<&mut ErasedMutPointer>,
    ) -> FfiSlice<&'a Entity>;
}
