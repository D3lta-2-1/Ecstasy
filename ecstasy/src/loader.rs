use std::sync::OnceLock;

use ecstasy_ffi::{
    ArchetypeIndex, ColumnIndex, Component, Entity, EntityLocation, EventIndex, LocalColumnIndex,
    QueryBuilder, QuerySetIndex, QuerySetOpaque, QuerySetVtable, RegistryError, RegistryOpaque,
    RegistryVtable, SystemContextVtable, TypeDescriptor, TypeIdentity,
};
use reflexion::{
    drop_location::DropLocation,
    erased::{ErasedMutPointer, ErasedRef},
    ffi_collection::FfiCollectionIter,
    ffi_slice::FfiSlice,
};

use ecstasy_ffi::{
    PublisherHandle, SchedulerBuilderOpaque, SchedulerBuilderVtable, SystemContextOpaque,
    SystemOpaque, SystemVtable,
};

// TODO: implement statically linked registry using cargo feature

#[repr(C)]
pub struct EcstasyContext {
    pub registry: &'static RegistryVtable,
    pub query_set: &'static QuerySetVtable,
    pub scheduler_builder: &'static SchedulerBuilderVtable,
    pub system_context: &'static SystemContextVtable,
}

/// a mapping to a the function of a registry, it can be both loaded from a DLL or resolved at compile time
pub struct Ecstasy;

#[derive(Debug)]
pub struct AlreadyInitializedError;

impl Ecstasy {
    /// call this function before any Ecstasy action in your dynamically linked binary
    /// panic if a context has already been provided
    pub fn load(context: EcstasyContext) -> Result<(), AlreadyInitializedError> {
        CONTEXT.set(context).map_err(|_| AlreadyInitializedError)
    }
}

static CONTEXT: OnceLock<EcstasyContext> = OnceLock::new();

pub struct RegistryLoader;
pub struct QuerySetLoader;
pub struct SchedulerBuilderLoader;
pub struct SystemContextLoader;

impl RegistryLoader {
    fn get() -> &'static RegistryVtable {
        CONTEXT.get().expect("ecstasy not yet loaded").registry
    }

    pub fn find_or_register_component(
        opaque: &mut RegistryOpaque,
        component: &TypeDescriptor,
    ) -> Component {
        (Self::get().find_or_register_component)(opaque, component)
    }

    pub fn create_empty_entity(opaque: &mut RegistryOpaque) -> Entity {
        (Self::get().create_empty_entity)(opaque)
    }

    pub fn create_entity<'a, 'b>(
        opaque: &mut RegistryOpaque,
        components: impl Into<FfiSlice<&'b Component>>,
        values: FfiCollectionIter<DropLocation<'a>>,
    ) -> Entity {
        (Self::get().create_entity)(opaque, components.into(), values)
    }

    pub fn add_components<'s: 'a, 'a, 'b>(
        opaque: &'s mut RegistryOpaque,
        entity: Entity,
        components: impl Into<FfiSlice<&'b Component>>,
        values: FfiCollectionIter<DropLocation<'a>>,
    ) -> Result<(), RegistryError> {
        (Self::get().add_components)(opaque, entity, components.into(), values).into()
    }

    pub fn get_one_component(
        opaque: &RegistryOpaque,
        entity: Entity,
        identity: TypeIdentity,
    ) -> Result<ErasedRef<'_>, RegistryError> {
        (Self::get().get_one_component)(opaque, entity, identity).into()
    }

    pub fn location(
        opaque: &RegistryOpaque,
        entity: Entity,
    ) -> Result<EntityLocation, RegistryError> {
        (Self::get().location)(opaque, entity).into()
    }

    pub fn get_query_id(opaque: &mut RegistryOpaque, builder: QueryBuilder) -> QuerySetIndex {
        (Self::get().get_query_id)(opaque, builder)
    }

    pub fn get_query<'a>(opaque: &RegistryOpaque, id: QuerySetIndex) -> &QuerySetOpaque {
        (Self::get().get_query)(opaque, id)
    }

    pub unsafe fn get_column_begin<'a, 'b, 'c>(
        opaque: &'a RegistryOpaque,
        archetype_index: ArchetypeIndex,
        columns: impl Into<FfiSlice<&'b ColumnIndex>>,
        starts: impl Into<FfiSlice<&'c mut ErasedMutPointer>>,
    ) -> &'a [Entity] {
        unsafe {
            (Self::get().get_column_begin)(opaque, archetype_index, columns.into(), starts.into())
                .into()
        }
    }
}

impl QuerySetLoader {
    fn get() -> &'static QuerySetVtable {
        CONTEXT.get().expect("ecstasy not yet loaded").query_set
    }

    pub fn get_local_column_index(
        opaque: &QuerySetOpaque,
        identity: &TypeIdentity,
    ) -> LocalColumnIndex {
        (Self::get().get_local_column_index)(opaque, identity)
    }

    pub fn columns_index_for_archetype(
        opaque: &QuerySetOpaque,
        archetype_index: ArchetypeIndex,
    ) -> Result<&[ColumnIndex], RegistryError> {
        let result: Result<_, _> =
            (Self::get().columns_index_for_archetype)(opaque, archetype_index).into();
        result.map(|slice| slice.into())
    }
}

impl SchedulerBuilderLoader {
    fn get() -> &'static SchedulerBuilderVtable {
        CONTEXT
            .get()
            .expect("ecstasy not yet loaded")
            .scheduler_builder
    }

    pub fn registry(opaque: &mut SchedulerBuilderOpaque) -> &mut RegistryOpaque {
        (Self::get().registry)(opaque)
    }

    pub fn find_event(opaque: &mut SchedulerBuilderOpaque, event: TypeDescriptor) -> EventIndex {
        (Self::get().find_event)(opaque, event)
    }

    pub fn add_system(
        opaque: &mut SchedulerBuilderOpaque,
        system: DropLocation<SystemOpaque>,
        vtable: &'static SystemVtable,
    ) {
        (Self::get().add_system)(opaque, system, vtable)
    }
}

impl SystemContextLoader {
    fn get() -> &'static SystemContextVtable {
        CONTEXT
            .get()
            .expect("ecstasy not yet loaded")
            .system_context
    }

    pub fn registry(opaque: &SystemContextOpaque) -> &RegistryOpaque {
        (Self::get().registry)(opaque)
    }

    pub unsafe fn get_publisher(
        opaque: &SystemContextOpaque,
        event: EventIndex,
    ) -> PublisherHandle<'_> {
        unsafe { (Self::get().get_publisher)(opaque, event) }
    }
}
