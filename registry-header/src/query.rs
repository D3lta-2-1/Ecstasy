use reflexion::{erased::ErasedMutPointer, ffi_slice::FfiSlice};
use registry_ffi::{
    ArchetypeIndex, ColumnIndex, ComponentDescriptor, Entity, EntityLocation, LocalColumnIndex,
    QueryIndex, RegistryMutHandle,
};
use std::array;

use crate::Component;

trait ComponentRef<'a> {
    const IS_MUT: bool; //TODO: add support for mutability
    const DESCRIPTOR: ComponentDescriptor;
    unsafe fn from_erased(ptr: ErasedMutPointer) -> Self;
}

impl<T: Component> ComponentRef<'_> for &T {
    const IS_MUT: bool = false;
    const DESCRIPTOR: ComponentDescriptor = T::DESCRIPTOR;
    unsafe fn from_erased(ptr: ErasedMutPointer) -> Self {
        unsafe { ptr.as_erased_ref().cast() }
    }
}

impl<T: Component> ComponentRef<'_> for &mut T {
    const IS_MUT: bool = false;
    const DESCRIPTOR: ComponentDescriptor = T::DESCRIPTOR;

    unsafe fn from_erased(ptr: ErasedMutPointer) -> Self {
        unsafe { ptr.as_erased_mut().cast() }
    }
}

//TODO: this might need to be moved elsewhere
pub trait StaticCollection<T>: AsRef<[T]> + AsMut<[T]> {
    // used to avoid to use an explicit SIZE generic...
    fn from_fn(f: impl Fn(usize) -> T) -> Self;
    fn for_each(&mut self, f: impl Fn(&mut T));
}

impl<T, const SIZE: usize> StaticCollection<T> for [T; SIZE] {
    fn from_fn(f: impl Fn(usize) -> T) -> Self {
        array::from_fn(f)
    }

    fn for_each(&mut self, f: impl Fn(&mut T)) {
        self.iter_mut().for_each(f)
    }
}

pub trait QueryBundle {
    type TDescriptors: StaticCollection<ComponentDescriptor>;
    type TPointers: StaticCollection<ErasedMutPointer>;
    type Array<T>: StaticCollection<T>;
    const DESCRIPTORS: Self::TDescriptors; //descriptor of the value, not the refs
    fn build(pointers: Self::TPointers) -> Self;
}

impl<'a, T: ComponentRef<'a>, U: ComponentRef<'a>> QueryBundle for (T, U) {
    type TDescriptors = [ComponentDescriptor; 2];
    type TPointers = [ErasedMutPointer; 2];
    type Array<V> = [V; 2];
    const DESCRIPTORS: [ComponentDescriptor; 2] = [T::DESCRIPTOR, U::DESCRIPTOR];

    fn build([u, v]: [ErasedMutPointer; 2]) -> Self {
        unsafe { (T::from_erased(u), U::from_erased(v)) }
    }
}

pub struct Query<QUERY: QueryBundle> {
    id: QueryIndex,
    local_to_column_index: QUERY::Array<LocalColumnIndex>, // the ordering used here is the same the bundle fields
}

// TODO: improve builder interface,
impl<QUERY: QueryBundle> Query<QUERY> {
    pub fn new(registry: &mut RegistryMutHandle) -> Self {
        let mut requested_components: Vec<_> = QUERY::DESCRIPTORS
            .as_ref()
            .iter()
            .map(|c| registry.find_or_register_component(c))
            .collect();
        requested_components.sort();
        let id = registry.get_query_id(requested_components.as_slice().into());
        let columns = <QUERY::Array<LocalColumnIndex>>::from_fn(|i| {
            registry.query_get_local_column_index(id, &QUERY::DESCRIPTORS.as_ref()[i].identity)
        });
        Self {
            id,
            local_to_column_index: columns,
        }
    }

    /// return the corresponding column, properly ordered for reading, return none if the archetype isn't part of the query
    fn get_columns_in_archetype(
        &self,
        registry: &mut RegistryMutHandle,
        archetype_index: ArchetypeIndex,
    ) -> Option<QUERY::Array<ColumnIndex>> {
        let option: Option<FfiSlice<&ColumnIndex>> = registry
            .query_get_columns_index(self.id, archetype_index)
            .into();
        let columns = option?;
        Some(<QUERY::Array<ColumnIndex>>::from_fn(|i| {
            columns[self.local_to_column_index.as_ref()[i]]
        }))
    }

    pub fn get(&self, registry: &mut RegistryMutHandle, entity: Entity) -> Option<QUERY> {
        let option: Option<EntityLocation> = registry.location(entity).into();
        let EntityLocation {
            archetype_index,
            entity_index,
        } = option?;
        let columns = self.get_columns_in_archetype(registry, archetype_index)?;
        let mut starts = <QUERY::TPointers>::from_fn(|_| ErasedMutPointer::empty());
        unsafe {
            registry.get_colum_begin(
                archetype_index,
                columns.as_ref().into(),
                starts.as_mut().into(),
            );
            starts.for_each(|p| *p = p.offset(entity_index));
            Some(QUERY::build(starts))
        }
    }
}
