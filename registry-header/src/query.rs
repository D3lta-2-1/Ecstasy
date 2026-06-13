use std::array;

use reflexion::erased::ErasedMutPointer;
use registry_ffi::{
    ArchetypeIndex, ColumnIndex, ComponentDescriptor, ComponentMutability, Entity, EntityLocation,
    LocalColumnIndex, QueryBuilder, RegistryError, RegistryHandle, RegistryMutHandle,
};

use crate::Component;

pub trait ComponentRef {
    const MUTABILITY: ComponentMutability;
    type Inner;
    type Ref<'a>;
    unsafe fn from_erased<'a>(ptr: ErasedMutPointer) -> Self::Ref<'a>;
}

impl<T: 'static> ComponentRef for &T {
    const MUTABILITY: ComponentMutability = ComponentMutability::Const;
    type Inner = T;
    type Ref<'a> = &'a T;
    unsafe fn from_erased<'a>(ptr: ErasedMutPointer) -> &'a T {
        unsafe { ptr.as_erased_ref().cast() }
    }
}

impl<T: Component + 'static> ComponentRef for &mut T {
    const MUTABILITY: ComponentMutability = ComponentMutability::Mut;
    type Inner = T;
    type Ref<'a> = &'a mut T;
    unsafe fn from_erased<'a>(ptr: ErasedMutPointer) -> &'a mut T {
        unsafe { ptr.as_erased_mut().cast() }
    }
}

// this might need to be moved elsewhere
pub trait StaticCollection<T>: AsRef<[T]> + AsMut<[T]> {
    // used to avoid to use an explicit SIZE generic...
    fn from_fn(f: impl FnMut(usize) -> T) -> Self;
    fn for_each(&mut self, f: impl FnMut(&mut T));
}

impl<T, const SIZE: usize> StaticCollection<T> for [T; SIZE] {
    fn from_fn(f: impl FnMut(usize) -> T) -> Self {
        array::from_fn(f)
    }

    fn for_each(&mut self, f: impl FnMut(&mut T)) {
        self.iter_mut().for_each(f)
    }
}

pub trait QueryBundle {
    type BundleRef<'a>;
    type Array<T: 'static>: StaticCollection<T>;
    const DESCRIPTORS: Self::Array<ComponentDescriptor>; //descriptor of the value, not the refs
    const MUTABILTY: Self::Array<ComponentMutability>;
    unsafe fn build<'a>(pointers: Self::Array<ErasedMutPointer>) -> Self::BundleRef<'a>;
}

impl<T, U> QueryBundle for (T, U)
where
    T: ComponentRef,
    T::Inner: Component,
    U: ComponentRef,
    U::Inner: Component,
{
    type BundleRef<'a> = (T::Ref<'a>, U::Ref<'a>);
    type Array<V: 'static> = [V; 2];
    const DESCRIPTORS: [ComponentDescriptor; 2] = [T::Inner::DESCRIPTOR, U::Inner::DESCRIPTOR];
    const MUTABILTY: [ComponentMutability; 2] = [T::MUTABILITY, U::MUTABILITY];

    unsafe fn build<'a>([u, v]: [ErasedMutPointer; 2]) -> Self::BundleRef<'a> {
        unsafe { (T::from_erased::<'a>(u), U::from_erased::<'a>(v)) }
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

pub struct QueryState<Bundle: QueryBundle> {
    pub id: registry_ffi::Query,
    local_to_column_index: Bundle::Array<LocalColumnIndex>, // the ordering used here is the same the bundle fields, meaning that local_to_column_index[0] give the local column index of the first component
}

// TODO: add iterator on Queries,
impl<Bundle: QueryBundle> QueryState<Bundle> {
    pub fn new(registry: &mut RegistryMutHandle) -> Self {
        let mut requested_components = <Bundle::Array<Entity>>::from_fn(|i| {
            registry.find_or_register_component(&Bundle::DESCRIPTORS.as_ref()[i])
        });
        let mut mutabilities = Bundle::MUTABILTY;
        sort(requested_components.as_mut(), mutabilities.as_mut());

        let builder = QueryBuilder {
            requested_components: requested_components.as_ref().into(),
            mutabilities: mutabilities.as_ref().into(),
        };

        let id = registry.get_query_id(builder);
        let columns = <Bundle::Array<LocalColumnIndex>>::from_fn(|i| {
            let query = registry.get_query(id.set);
            query.get_local_column_index(&Bundle::DESCRIPTORS.as_ref()[i].identity)
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
    ) -> Result<Bundle::Array<ColumnIndex>, RegistryError> {
        let query = registry.get_query(self.id.set);
        let columns = query
            .columns_index_for_archetype(archetype_index)
            .as_result()?;
        Ok(<Bundle::Array<ColumnIndex>>::from_fn(|i| {
            columns[self.local_to_column_index.as_ref()[i].0]
        }))
    }

    pub fn get<'a>(
        &'a self,
        registry: RegistryHandle,
        entity: Entity,
    ) -> Result<Bundle::BundleRef<'a>, RegistryError> {
        let EntityLocation {
            archetype_index,
            entity_index,
        } = registry.location(entity).as_result()?;
        let columns = self.get_columns_in_archetype(registry, archetype_index)?;
        let mut starts = <Bundle::Array<ErasedMutPointer>>::from_fn(|_| ErasedMutPointer::empty());
        unsafe {
            registry.get_column_begin(
                archetype_index,
                columns.as_ref().into(),
                starts.as_mut().into(),
            );
            starts.for_each(|p| *p = p.offset(entity_index.0));
            Ok(Bundle::build(starts))
        }
    }

    pub fn promote<'registry, 'state>(
        &'state self,
        handle: RegistryHandle<'registry>,
    ) -> Query<'registry, 'state, Bundle> {
        Query {
            inner: &self,
            handle,
        }
    }
}

pub struct Query<'registry, 'state, Bundle: QueryBundle> {
    inner: &'state QueryState<Bundle>,
    handle: RegistryHandle<'registry>,
}

impl<'registry, 'state, Bundle: QueryBundle> Query<'registry, 'state, Bundle> {
    pub fn get(&self, entity: Entity) -> Result<Bundle::BundleRef<'_>, RegistryError> {
        self.inner.get(self.handle, entity)
    }
}
