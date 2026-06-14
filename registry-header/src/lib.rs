pub mod bundle;
pub mod query;
pub mod system;
pub mod array_utils;

pub use crate::bundle::{Component, StaticBundle};
use reflexion::ffi_collection::FfiCollectionIter;
use registry_ffi::{Entity, RegistryError, RegistryMutHandle};

/// the registry header is the final interface between the ECS internal and "external" world.
/// it's where all clean generic methods are defined
/// each binary accessing the ECS across DLL boundaries will get a copy of all this code and data structure
/// it's the perfect place to some target local caching such as type_id <-> component identity
pub struct RegistryHeader<'a> {
    registry: RegistryMutHandle<'a>,
}

impl<'a> RegistryHeader<'a> {
    pub fn new(handle: RegistryMutHandle<'a>) -> Self {
        Self { registry: handle }
    }

    pub fn mut_handle(&mut self) -> &mut RegistryMutHandle<'a> {
        &mut self.registry
    }

    pub fn new_entity<const SIZE: usize, T: StaticBundle<SIZE>>(&mut self, bundle: T) -> Entity {
        let mut component: [Entity; SIZE] =
            std::array::from_fn(|i| self.registry.find_or_register_component(&T::DESCRIPTORS[i]));
        let mut permutation = permutation::sort(&component); //this permutation could be stored to spare some calculation...
        bundle.read(|mut locations| {
            permutation.apply_slice_in_place(&mut component);
            permutation.apply_slice_in_place(&mut locations);

            FfiCollectionIter::from_array(locations, |iter| {
                self.registry
                    .create_entity(component.as_slice().into(), iter)
            })
        })
    }

    pub fn add<const SIZE: usize, T: StaticBundle<SIZE>>(
        &mut self,
        entity: Entity,
        bundle: T,
    ) -> Result<(), RegistryError> {
        let mut component: [Entity; SIZE] =
            std::array::from_fn(|i| self.registry.find_or_register_component(&T::DESCRIPTORS[i]));

        let mut permutation = permutation::sort(&component); //this permutation could be stored to spare some calculation...
        bundle.read(|mut locations| {
            permutation.apply_slice_in_place(&mut component);
            permutation.apply_slice_in_place(&mut locations);

            FfiCollectionIter::from_array(locations, |iter| {
                self.registry
                    .add_components(entity, component.as_slice().into(), iter)
                    .into()
            })
        })
    }

    pub fn get<T: Component>(&self, entity: Entity) -> Result<&T, RegistryError> {
        self.registry
            .get_one_component(entity, T::DESCRIPTOR.identity)
            .as_result()
            .map(|c| c.cast::<T>())
    }
}
