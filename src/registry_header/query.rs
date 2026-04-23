use crate::registry::{ArchetypeIndex, ColumnIndex, EntityLocation, LocalColumIndex, QueryIndex};
use crate::registry_header::{Component, RegistryHeader};
use crate::shared::id::{ComponentDescriptor, Entity};
use reflexion::erased::ErasedMutPointer;
use std::array;

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
    local_to_column_index: QUERY::Array<LocalColumIndex>, // the ordering used here is the same the bundle fields
}

// TODO: improve builder interface,
impl<QUERY: QueryBundle> Query<QUERY> {
    pub fn new(header: &mut RegistryHeader) -> Self {
        let mut requested_components: Vec<_> = QUERY::DESCRIPTORS
            .as_ref()
            .iter()
            .map(|c| header.registry.find_or_register_component(c))
            .collect();
        requested_components.sort();
        let id = header.registry.get_query_id(&requested_components);
        let columns = <QUERY::Array<LocalColumIndex>>::from_fn(|i| {
            header
                .registry
                .query_get_local_column_index(id, &QUERY::DESCRIPTORS.as_ref()[i].identity)
        });
        Self {
            id,
            local_to_column_index: columns,
        }
    }

    /// return the corresponding column, properly ordered for reading, return none if the archetype isn't part of the query
    fn get_columns_in_archetype(
        &self,
        header: &RegistryHeader,
        archetype_index: ArchetypeIndex,
    ) -> Option<QUERY::Array<ColumnIndex>> {
        let columns = header
            .registry
            .query_get_columns_index(self.id, archetype_index)?;
        Some(<QUERY::Array<ColumnIndex>>::from_fn(|i| {
            columns[self.local_to_column_index.as_ref()[i]]
        }))
    }

    pub fn get(&self, header: &RegistryHeader, entity: Entity) -> Option<QUERY> {
        let EntityLocation {
            archetype_index,
            entity_index,
        } = header.registry.location(entity)?;
        let columns = self.get_columns_in_archetype(header, archetype_index)?;
        let mut starts = <QUERY::TPointers>::from_fn(|_| ErasedMutPointer::empty());
        unsafe {
            header
                .registry
                .get_colum_begin(archetype_index, columns.as_ref(), starts.as_mut());
            starts.for_each(|p| *p = p.offset(entity_index));
            Some(QUERY::build(starts))
        }
    }
}
