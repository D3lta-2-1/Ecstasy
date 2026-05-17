use reflexion::{
    drop_location::DropLocation,
    erased::{ErasedMutPointer, ErasedRef},
    ffi_collection::FfiCollectionIter,
    ffi_enum::FfiResult,
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
/// - Generation 0 is never used, and reserved for future uses.
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
/// Position of a column *inside* a ``Query``
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
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct ComponentIdentity {
    pub path: FfiSlice<&'static u8>,
    pub name: FfiSlice<&'static u8>,
}

impl Debug for ComponentIdentity {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let path = str::from_utf8(&self.path).unwrap_or("[non valid utf8]");
        let name = str::from_utf8(&self.name).unwrap_or("[non valid utf8]");

        f.debug_struct("ComponentIdentity")
            .field("path", &path)
            .field("name", &name)
            .finish()
    }
}

impl ComponentIdentity {
    pub const EMPTY: Self = Self {
        path: FfiSlice::from_str("ecstasy"),
        name: FfiSlice::from_str("ecstasy"),
    };
}

#[derive(Copy, Clone, Debug)]
pub struct ComponentDescriptor {
    pub identity: ComponentIdentity,
    pub type_info: TypeInfo,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum RegistryError {
    EntityNotFound,
    ComponentNotFound,
    ArchetypeNotFound,
}

#[repr(C)]
pub struct QueryBuilder<'a> {
    /// both slices must be the same size, on contain
    requested_components: FfiSlice<&'a Component>,
    mutabilities: FfiSlice<&'a bool>,
}

#[repr(C)]
pub struct SystemBuilder<'a> {
    queries: FfiSlice<&'a QueryBuilder<'a>>,
    executor: extern "C" fn(PluginHandle, FfiSlice<QueryHandle>),
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
    ) -> FfiResult<(), RegistryError>;
    /// get a component from its Identity
    // TODO: consider update ComponentIdentity for Component
    extern "C" fn get_one_component(
        &self,
        entity: Entity,
        identity: ComponentIdentity,
    ) -> FfiResult<ErasedRef<'_>, RegistryError>;
    extern "C" fn location(&self, entity: Entity) -> FfiResult<EntityLocation, RegistryError>;

    /// this function will return the query ID associated with this builder, and create if required
    /// Queries can't be deleted, they are meant to be used through systems
    extern "C" fn get_query_id(&mut self, requested_components: FfiSlice<&Component>)
    -> QueryIndex;

    extern "C" fn get_query<'a>(&'a self, id: QueryIndex) -> QueryHandle<'a>;

    ///write the start of the asked column in the ``start`` parameter, and provide a slice on the associated Entities
    unsafe extern "C" fn get_column_begin<'a>(
        &'a self,
        archetype_index: ArchetypeIndex,
        columns: FfiSlice<&ColumnIndex>,
        starts: FfiSlice<&mut ErasedMutPointer>,
    ) -> FfiSlice<&'a Entity>;
}

/// Queries are a way to retrieve all entities/archetypes that match a given set of components
/// They are able to iterate on all entities as well as doing random access
/// To improve performances, they define "local column", these local column match each archetype column in a static manner for easier access
/// each accessible component of the query gets its own local column
#[vtable]
pub trait Query {
    /// return the ``LocalColumnIndex`` of a Component, panic if it doesn't belong to the query
    extern "C" fn get_local_column_index(&self, identity: &ComponentIdentity) -> LocalColumnIndex;
    /// return an array that map the LocalColumnIndex to the real column in a given Archetype.
    extern "C" fn columns_index_for_archetype(
        &self,
        archetype_index: ArchetypeIndex,
    ) -> FfiResult<FfiSlice<&ColumnIndex>, RegistryError>;
}

/// A Plugin is an "endpoint in the ECS, it mainly store crate-local information"
#[vtable]
pub trait Plugin {}

#[vtable]
pub trait SystemExecutor {
    extern "C" fn call(&mut self, handle: PluginHandle, queries: FfiSlice<&QueryHandle>);
}
