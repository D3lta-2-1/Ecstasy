use reflexion::erased::ErasedMutPointer;
use registry_ffi::{
    ArchetypeIndex, ColumnIndex, ComponentDescriptor, ComponentMutability, Entity, EntityLocation,
    LocalColumnIndex, QueryBuilder, RegistryError, RegistryHandle, RegistryMutHandle,
};
use std::array;

use crate::Component;

trait ComponentRef<'a> {
    const MUTABILITY: ComponentMutability;
    const DESCRIPTOR: ComponentDescriptor;
    unsafe fn from_erased(ptr: ErasedMutPointer) -> Self;
}

impl<T: Component> ComponentRef<'_> for &T {
    const MUTABILITY: ComponentMutability = ComponentMutability::Const;
    const DESCRIPTOR: ComponentDescriptor = T::DESCRIPTOR;
    unsafe fn from_erased(ptr: ErasedMutPointer) -> Self {
        unsafe { ptr.as_erased_ref().cast() }
    }
}

impl<T: Component> ComponentRef<'_> for &mut T {
    const MUTABILITY: ComponentMutability = ComponentMutability::Mut;
    const DESCRIPTOR: ComponentDescriptor = T::DESCRIPTOR;

    unsafe fn from_erased(ptr: ErasedMutPointer) -> Self {
        unsafe { ptr.as_erased_mut().cast() }
    }
}

// this might need to be moved elsewhere
pub trait StaticCollection<T: Copy>: AsRef<[T]> + AsMut<[T]> + Copy {
    // used to avoid to use an explicit SIZE generic...
    fn from_fn(f: impl FnMut(usize) -> T) -> Self;
    fn for_each(&mut self, f: impl FnMut(&mut T));
}

impl<T: Copy, const SIZE: usize> StaticCollection<T> for [T; SIZE] {
    fn from_fn(f: impl FnMut(usize) -> T) -> Self {
        array::from_fn(f)
    }

    fn for_each(&mut self, f: impl FnMut(&mut T)) {
        self.iter_mut().for_each(f)
    }
}

pub trait QueryBundle {
    type Array<T: Copy>: StaticCollection<T>;
    const DESCRIPTORS: Self::Array<ComponentDescriptor>; //descriptor of the value, not the refs
    const MUTABILTY: Self::Array<ComponentMutability>;
    unsafe fn build(pointers: Self::Array<ErasedMutPointer>) -> Self;
}

impl<'a, T: ComponentRef<'a>, U: ComponentRef<'a>> QueryBundle for (T, U) {
    type Array<V: Copy> = [V; 2];
    const DESCRIPTORS: [ComponentDescriptor; 2] = [T::DESCRIPTOR, U::DESCRIPTOR];
    const MUTABILTY: [ComponentMutability; 2] = [T::MUTABILITY, U::MUTABILITY];

    unsafe fn build([u, v]: [ErasedMutPointer; 2]) -> Self {
        unsafe { (T::from_erased(u), U::from_erased(v)) }
    }
}

/// sort on array, and applies the same perumtation to the other one
pub fn sort<T: Ord, U>(keys: &mut [T], values: &mut [U]) {
    assert_eq!(keys.len(), values.len());
    for i in 1..keys.len() {
        for j in (0..i).rev() {
            if keys[j + 1] < keys[j] {
                keys.swap(j + 1, j);
                values.swap(j + 1, j);
            } else {
                break;
            }
        }
    }
}

#[test]
fn test_sort() {
    let mut keys = [5, 4, 3, 2, 1];
    let mut values = [1, 2, 3, 4, 5];
    sort(&mut keys, &mut values);
    assert_eq!(keys, [1, 2, 3, 4, 5]);
    assert_eq!(values, [5, 4, 3, 2, 1]);
}

#[derive(Clone, Copy)]
pub struct QueryHeaderData<QUERY: QueryBundle> {
    pub id: registry_ffi::Query,
    local_to_column_index: QUERY::Array<LocalColumnIndex>, // the ordering used here is the same the bundle fields, meaning that local_to_column_index[0] give the local column index of the first component
}

// TODO: add iterator on Queries,
impl<QUERY: QueryBundle> QueryHeaderData<QUERY> {
    pub fn new(registry: &mut RegistryMutHandle) -> Self {
        let mut requested_components = <QUERY::Array<Entity>>::from_fn(|i| {
            registry.find_or_register_component(&QUERY::DESCRIPTORS.as_ref()[i])
        });
        let mut mutabilities = QUERY::MUTABILTY;
        sort(requested_components.as_mut(), mutabilities.as_mut());

        let builder = QueryBuilder {
            requested_components: requested_components.as_ref().into(),
            mutabilities: mutabilities.as_ref().into(),
        };

        let id = registry.get_query_id(builder);
        let columns = <QUERY::Array<LocalColumnIndex>>::from_fn(|i| {
            let query = registry.get_query(id.set);
            query.get_local_column_index(&QUERY::DESCRIPTORS.as_ref()[i].identity)
        });
        Self {
            id,
            local_to_column_index: columns,
        }
    }

    /// return the corresponding column, properly ordered for reading, return none if the archetype isn't part of the query
    fn get_columns_in_archetype(
        &self,
        registry: RegistryHandle,
        archetype_index: ArchetypeIndex,
    ) -> Result<QUERY::Array<ColumnIndex>, RegistryError> {
        let query = registry.get_query(self.id.set);
        let columns = query
            .columns_index_for_archetype(archetype_index)
            .as_result()?;
        Ok(<QUERY::Array<ColumnIndex>>::from_fn(|i| {
            columns[self.local_to_column_index.as_ref()[i].0]
        }))
    }

    pub fn get(&self, registry: RegistryHandle, entity: Entity) -> Result<QUERY, RegistryError> {
        let EntityLocation {
            archetype_index,
            entity_index,
        } = registry.location(entity).as_result()?;
        let columns = self.get_columns_in_archetype(registry, archetype_index)?;
        let mut starts = <QUERY::Array<ErasedMutPointer>>::from_fn(|_| ErasedMutPointer::empty());
        unsafe {
            registry.get_column_begin(
                archetype_index,
                columns.as_ref().into(),
                starts.as_mut().into(),
            );
            starts.for_each(|p| *p = p.offset(entity_index.0));
            Ok(QUERY::build(starts))
        }
    }

    pub fn promote<'a>(&'a self, handle: RegistryHandle<'a>) -> Query<'a, QUERY> {
        Query {
            inner: &self,
            handle,
        }
    }
}

pub struct Query<'a, QUERY: QueryBundle> {
    inner: &'a QueryHeaderData<QUERY>,
    handle: RegistryHandle<'a>,
}

impl<'a, QUERY: QueryBundle> Query<'a, QUERY> {
    pub fn get(&self, entity: Entity) -> Result<QUERY, RegistryError> {
        self.inner.get(self.handle, entity)
    }
}
