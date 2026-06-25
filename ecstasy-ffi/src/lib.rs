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
    cmp::Ordering,
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

macro_rules! create_index {
    // This macro takes an argument of designator `ident` and
    // creates a function named `$func_name`.
    // The `ident` designator is used for variable/function names.
    ($type_name:ident, $doc:literal) => {
        #[repr(transparent)]
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        #[doc = $doc]
        pub struct $type_name(pub usize);

        impl From<$type_name> for usize {
            fn from($type_name(v): $type_name) -> Self {
                v
            }
        }

        impl From<usize> for $type_name {
            fn from(v: usize) -> Self {
                $type_name(v)
            }
        }
    };
}

create_index!(
    ArchetypeIndex,
    "unique identifier for an ``Archetype`` in the registry"
);
create_index!(ColumnIndex, "Index of a column in an ``Archetype``");
create_index!(EntityIndex, "Position of an Entity its ``Archetype``");
create_index!(
    LocalColumnIndex,
    "Position of a column *inside* a ``Query``"
);
create_index!(
    QuerySetIndex,
    "unique identifier for a ``QuerySet`` in the registry"
);
create_index!(EventIndex, "a unique identifier for an Event type");

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EntityLocation {
    pub archetype_index: ArchetypeIndex,
    pub entity_index: EntityIndex,
}

/// a fully qualified component identity, used to get ComponentData from a component path and name.
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct TypeIdentity {
    pub path: FfiSlice<&'static u8>,
    pub name: FfiSlice<&'static u8>,
}

impl Debug for TypeIdentity {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let path = str::from_utf8(&self.path).unwrap_or("[non valid utf8]");
        let name = str::from_utf8(&self.name).unwrap_or("[non valid utf8]");

        f.debug_struct("ComponentIdentity")
            .field("path", &path)
            .field("name", &name)
            .finish()
    }
}

impl TypeIdentity {
    pub const EMPTY: Self = Self {
        path: FfiSlice::from_str("ecstasy"),
        name: FfiSlice::from_str("ecstasy"),
    };
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct TypeDescriptor {
    pub identity: TypeIdentity,
    pub type_info: TypeInfo,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum RegistryError {
    EntityNotFound,
    ComponentNotFound,
    ArchetypeNotFound,
}

#[vtable]
pub trait Registry {
    /// Find the Entity that represent a given component
    extern "C-unwind" fn find_or_register_component(&mut self, component: &TypeDescriptor) -> Component;
    /// Create a new Entity id, the new entity can be queried in empty queries.
    extern "C-unwind" fn create_empty_entity(&mut self) -> Entity;
    extern "C-unwind" fn create_entity<'a>(
        &mut self,
        components: FfiSlice<&Component>,
        values: FfiCollectionIter<DropLocation<'a>>,
    ) -> Entity;
    extern "C-unwind" fn add_components<'s: 'a, 'a>(
        &'s mut self,
        entity: Entity,
        components: FfiSlice<&Component>,
        values: FfiCollectionIter<DropLocation<'a>>,
    ) -> FfiResult<(), RegistryError>;
    /// get a component from its Identity
    // TODO: consider update ComponentIdentity for Component
    extern "C-unwind" fn get_one_component(
        &self,
        entity: Entity,
        identity: TypeIdentity,
    ) -> FfiResult<ErasedRef<'_>, RegistryError>;
    extern "C-unwind" fn location(&self, entity: Entity) -> FfiResult<EntityLocation, RegistryError>;

    /// this function will return the query ID associated with this builder, and create if required
    /// Queries can't be deleted, they are meant to be used through systems
    extern "C-unwind" fn get_query_id(&mut self, builder: QueryBuilder) -> QuerySetIndex;

    extern "C-unwind" fn get_query(&self, id: QuerySetIndex) -> &QuerySetOpaque;

    ///write the start of the asked column in the ``start`` parameter, and provide a slice on the associated Entities
    unsafe extern "C-unwind" fn get_column_begin<'a>(
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
/// it's the caller responsibility to respects the mutability he asked for
#[vtable]
pub trait QuerySet {
    /// return the ``LocalColumnIndex`` of a Component, panic if it doesn't belong to the query
    extern "C-unwind" fn get_local_column_index(&self, identity: &TypeIdentity) -> LocalColumnIndex;
    /// return an array that map the LocalColumnIndex to the real column in a given Archetype.
    extern "C-unwind" fn columns_index_for_archetype(
        &self,
        archetype_index: ArchetypeIndex,
    ) -> FfiResult<FfiSlice<&ColumnIndex>, RegistryError>;
}

#[repr(C)]
pub struct QueryBuilder<'a> {
    /// both slices must be the same size, on contain
    pub requested_components: FfiSlice<&'a Component>,
}

#[vtable]
pub trait Producer {
    unsafe extern "C-unwind" fn push(&mut self, value: DropLocation);
}

#[vtable]
pub trait Consumer {
    // TODO: Add ErasedPointer (const version)
    unsafe extern "C-unwind" fn events(&self, len: &mut usize) -> ErasedMutPointer;
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ComponentMutability {
    Const,
    Mut,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EventUsage {
    Producer,
    Consumer,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BorrowedResource {
    Event {
        event: EventIndex,
        usage: EventUsage,
    },
    Component {
        component: Component,
        mutability: ComponentMutability,
    },
}

impl Ord for BorrowedResource {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Self::Event { .. }, Self::Component { .. }) => Ordering::Less,
            (Self::Component { .. }, Self::Event { .. }) => Ordering::Greater,
            (
                Self::Event {
                    event: self_event,
                    usage: self_usage,
                },
                Self::Event {
                    event: other_event,
                    usage: other_usage,
                },
            ) => self_event
                .cmp(other_event)
                .then(self_usage.cmp(other_usage)),
            (
                Self::Component {
                    component: self_component,
                    mutability: self_mutability,
                },
                Self::Component {
                    component: other_component,
                    mutability: other_mutability,
                },
            ) => self_component
                .cmp(other_component)
                .then(self_mutability.cmp(other_mutability)),
        }
    }
}

impl PartialOrd for BorrowedResource {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Used to create and store system before they are ran as a scheduler.
#[vtable]
pub trait SchedulerBuilder {
    /// mainly used to during system creation
    extern "C-unwind" fn registry(&mut self) -> &mut RegistryOpaque;
    extern "C-unwind" fn find_event(&mut self, event: TypeDescriptor) -> EventIndex;
    extern "C-unwind" fn add_system(
        &mut self,
        system: DropLocation<SystemOpaque>,
        vtable: &'static SystemVtable,
    );
}

#[vtable]
pub trait SystemContext {
    extern "C-unwind" fn registry(&self) -> &RegistryOpaque;
    unsafe extern "C-unwind" fn get_publisher(&self, event: EventIndex) -> &mut ProducerOpaque;
    unsafe extern "C-unwind" fn get_consumer(&self, event: EventIndex) -> &ConsumerOpaque;
}

/// Used to represent a system in the ECS, the system have to be honest, and precisely use the Queries it asked for
#[vtable]
pub trait System {
    extern "C-unwind" fn call(&mut self, context: &SystemContextOpaque);
    // return a slice
    extern "C-unwind" fn borrowed_resources(&self) -> FfiSlice<&BorrowedResource>;
}
