use std::array;
use std::marker::PhantomData;
use reflexion::erased::ErasedMutPointer;
use crate::registry::{ArchetypeIndex, ColumnIndex, EntityLocation, LocalColumIndex, QueryIndex};
use crate::registry_header::{Component, RegistryHeader};
use crate::shared::id::{ComponentDescriptor, Entity};

trait ComponentRef<'a> {
    const IS_MUT: bool;
    const DESCRIPTOR: ComponentDescriptor;
    unsafe fn from_erased(ptr: ErasedMutPointer) -> Self;
}

impl<T: Component> ComponentRef<'_> for &T {
    const IS_MUT: bool = false;
    const DESCRIPTOR: ComponentDescriptor = T::DESCRIPTOR;
    unsafe fn from_erased(ptr: ErasedMutPointer) -> Self {
        unsafe {
            ptr.as_erased_ref().cast()
        }
    }
}

impl<T: Component> ComponentRef<'_> for &mut T {
    const IS_MUT: bool = false;
    const DESCRIPTOR: ComponentDescriptor = T::DESCRIPTOR;

    unsafe fn from_erased(ptr: ErasedMutPointer) -> Self {
        unsafe {
            ptr.as_erased_mut().cast()
        }
    }
}

pub trait QueryBundle<const SIZE: usize> {
    const DESCRIPTORS: [ComponentDescriptor; SIZE]; //descriptor of the value, not the refs
    fn build(pointers: [ErasedMutPointer; SIZE]) -> Self;
}

impl<'a, T: ComponentRef<'a>, U: ComponentRef<'a>> QueryBundle<2> for (T, U) {
    const DESCRIPTORS: [ComponentDescriptor; 2] = [T::DESCRIPTOR, U::DESCRIPTOR];

    fn build([u, v]: [ErasedMutPointer; 2]) -> Self {
        unsafe { (T::from_erased(u), U::from_erased(v)) }
    }
}

pub struct Query<QUERY: QueryBundle<SIZE>, const SIZE: usize> {
    phantom: PhantomData<QUERY>,
    id: QueryIndex,
    local_to_column_index: [LocalColumIndex; SIZE] // the ordering used here is the same the bundle fields
}

// TODO: improve builder interface
impl<QUERY: QueryBundle<SIZE>, const SIZE: usize> Query<QUERY, SIZE> {
    pub fn new(header: &mut RegistryHeader) -> Self<> {
        let mut requested_components: Vec<_> =  QUERY::DESCRIPTORS.iter().map(|c| header.registry.find_or_register_component(c)).collect();
        requested_components.sort();
        let id = header.registry.get_query_id(&requested_components);
        let columns = array::from_fn(|i| header.registry.query_get_local_column_index(id, &QUERY::DESCRIPTORS[i].identity));
        Self {
            phantom: PhantomData,
            id,
            local_to_column_index: columns,
        }
    }

    /// return the corresponding column, properly ordered for reading, return none if the archetype isn't part of the query
    fn get_columns_in_archetype(&self, header: &RegistryHeader, archetype_index: ArchetypeIndex) -> Option<[ColumnIndex; SIZE]> {
        let columns = header.registry.query_get_columns_index(self.id, archetype_index)?;
        Some(array::from_fn(|i| columns[self.local_to_column_index[i]]))
    }

    pub fn get(&self, header: &RegistryHeader, entity: Entity) -> Option<QUERY> {
        let EntityLocation{
            archetype_index, entity_index
        } = header.registry.location(entity)?;
        let columns = self.get_columns_in_archetype(header, archetype_index)?;
        let mut starts= [ErasedMutPointer::empty(); SIZE];
        unsafe {
            header.registry.get_colum_begin(archetype_index, &columns, &mut starts);
            let pointers  = starts.map(|p| p.offset(entity_index));
            Some(QUERY::build(pointers))
        }
    }
}