use ecstasy_ffi::{
    ArchetypeIndex, BorrowedResource, ColumnIndex, ComponentMutability, Entity, EntityLocation,
    LocalColumnIndex, QueryBuilder, QuerySetIndex, RegistryError, RegistryOpaque, TypeDescriptor,
};
use reflexion::erased::ErasedMutPointer;

use crate::{
    Component,
    array_utils::Array,
    loader::{QuerySetLoader, RegistryLoader},
};

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

pub trait QueryBundle {
    type BundleRef<'a>;
    type Array<T: 'static + Copy>: Array<T> + Copy;
    const DESCRIPTORS: Self::Array<TypeDescriptor>; //descriptor of the value, not the refs
    const MUTABILTY: Self::Array<ComponentMutability>;
    unsafe fn build<'a>(pointers: Self::Array<ErasedMutPointer>) -> Self::BundleRef<'a>;
}

// TODO: implement Bundle for any size

impl<T, U> QueryBundle for (T, U)
where
    T: ComponentRef,
    T::Inner: Component,
    U: ComponentRef,
    U::Inner: Component,
{
    type BundleRef<'a> = (T::Ref<'a>, U::Ref<'a>);
    type Array<V: 'static + Copy> = [V; 2];
    const DESCRIPTORS: [TypeDescriptor; 2] = [T::Inner::DESCRIPTOR, U::Inner::DESCRIPTOR];
    const MUTABILTY: [ComponentMutability; 2] = [T::MUTABILITY, U::MUTABILITY];

    unsafe fn build<'a>([u, v]: [ErasedMutPointer; 2]) -> Self::BundleRef<'a> {
        unsafe { (T::from_erased::<'a>(u), U::from_erased::<'a>(v)) }
    }
}

pub struct QueryState<Bundle: QueryBundle> {
    pub id: QuerySetIndex,
    borrowed_resources: Bundle::Array<BorrowedResource>,
    local_to_column_index: Bundle::Array<LocalColumnIndex>, // the ordering used here is the same the bundle fields, meaning that local_to_column_index[0] give the local column index of the first component
}

// TODO: add iterator on Queries,
impl<Bundle: QueryBundle> QueryState<Bundle> {
    pub fn new(registry: &mut RegistryOpaque) -> Self {
        let mut borrowed_resources =
            <Bundle::Array<BorrowedResource>>::from_fn(|i| BorrowedResource::Component {
                mutability: Bundle::MUTABILTY[i],
                component: RegistryLoader::find_or_register_component(
                    registry,
                    &Bundle::DESCRIPTORS[i],
                ),
            });
        borrowed_resources.as_mut().sort();
        Self::ensure_query_validity(borrowed_resources.as_ref());

        let requested_components: Bundle::Array<_> = borrowed_resources.map(|resource| {
            let BorrowedResource::Component { component, .. } = resource else {
                unreachable!()
            };
            component
        });

        let builder = QueryBuilder {
            requested_components: requested_components.as_ref().into(),
        };

        let id = RegistryLoader::get_query_id(registry, builder);
        let query = RegistryLoader::get_query(registry, id);
        let columns = Bundle::DESCRIPTORS
            .map(|descriptor| QuerySetLoader::get_local_column_index(query, &descriptor.identity));
        Self {
            id,
            borrowed_resources,
            local_to_column_index: columns,
        }
    }

    pub fn ensure_query_validity(resources: &[BorrowedResource]) {
        for pair in resources.array_windows::<2>() {
            match pair {
                [
                    BorrowedResource::Component { component: a, .. },
                    BorrowedResource::Component { component: b, .. },
                ] => {
                    assert_ne!(a, b, "this query contain the same component twice");
                }
                _ => unreachable!(),
            }
        }
    }

    pub fn get_borrowed_resources(&self) -> &[BorrowedResource] {
        self.borrowed_resources.as_ref()
    }

    /// return the corresponding column, properly ordered for reading, return none if the archetype isn't part of the query
    fn get_columns_in_archetype(
        &self,
        registry: &RegistryOpaque,
        archetype_index: ArchetypeIndex,
    ) -> Result<Bundle::Array<ColumnIndex>, RegistryError> {
        let query = RegistryLoader::get_query(registry, self.id);
        let columns = QuerySetLoader::columns_index_for_archetype(query, archetype_index)?;
        Ok(<Bundle::Array<ColumnIndex>>::from_fn(|i| {
            columns[self.local_to_column_index.as_ref()[i].0]
        }))
    }

    pub fn get<'a>(
        &'a self,
        registry: &RegistryOpaque,
        entity: Entity,
    ) -> Result<Bundle::BundleRef<'a>, RegistryError> {
        let EntityLocation {
            archetype_index,
            entity_index,
        } = RegistryLoader::location(registry, entity)?;
        let columns = self.get_columns_in_archetype(registry, archetype_index)?;
        let mut starts = <Bundle::Array<ErasedMutPointer>>::from_fn(|_| ErasedMutPointer::empty());
        unsafe {
            RegistryLoader::get_column_begin(
                registry,
                archetype_index,
                columns.as_ref(),
                starts.as_mut(),
            );
            starts.for_each(|p| *p = p.offset(entity_index.0));
            Ok(Bundle::build(starts))
        }
    }

    pub fn promote<'registry, 'state>(
        &'state self,
        handle: &'registry RegistryOpaque,
    ) -> Query<'registry, 'state, Bundle> {
        Query {
            inner: &self,
            handle,
        }
    }
}

pub struct Query<'registry, 'state, Bundle: QueryBundle> {
    inner: &'state QueryState<Bundle>,
    handle: &'registry RegistryOpaque,
}

impl<'registry, 'state, Bundle: QueryBundle> Query<'registry, 'state, Bundle> {
    pub fn get(&self, entity: Entity) -> Result<Bundle::BundleRef<'_>, RegistryError> {
        self.inner.get(self.handle, entity)
    }
}
