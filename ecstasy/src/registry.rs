use crate::{
    bundle::{Component, StaticBundle},
    loader::RegistryLoader,
};
use ecstasy_ffi::{Entity, RegistryError, RegistryOpaque};
use reflexion::ffi_collection::FfiCollectionIter;

/// the registry header is the final interface between the ECS internal and "external" world.
/// it's where all clean generic methods are defined
/// each binary accessing the ECS across DLL boundaries will get a copy of all this code and data structure
/// it's the perfect place to some target local caching such as type_id <-> component identity
pub struct RegistryHeader<'a> {
    registry: &'a mut RegistryOpaque,
}

impl<'a> RegistryHeader<'a> {
    pub fn new(handle: &'a mut RegistryOpaque) -> Self {
        Self { registry: handle }
    }

    pub fn registry<'b>(&'b mut self) -> &'b mut RegistryOpaque {
        self.registry
    }

    pub fn new_entity<const SIZE: usize, T: StaticBundle<SIZE>>(&mut self, bundle: T) -> Entity {
        let mut component: [Entity; SIZE] = std::array::from_fn(|i| {
            RegistryLoader::find_or_register_component(self.registry, &T::DESCRIPTORS[i])
        });
        let mut permutation = permutation::sort(&component); //this permutation could be stored to spare some calculation...
        bundle.read(|mut locations| {
            permutation.apply_slice_in_place(&mut component);
            permutation.apply_slice_in_place(&mut locations);

            FfiCollectionIter::from_array(locations, |iter| {
                RegistryLoader::create_entity(self.registry, component.as_slice(), iter)
            })
        })
    }

    pub fn add<const SIZE: usize, T: StaticBundle<SIZE>>(
        &mut self,
        entity: Entity,
        bundle: T,
    ) -> Result<(), RegistryError> {
        let mut component: [Entity; SIZE] = std::array::from_fn(|i| {
            RegistryLoader::find_or_register_component(self.registry, &T::DESCRIPTORS[i])
        });

        let mut permutation = permutation::sort(&component); //this permutation could be stored to spare some calculation...
        bundle.read(|mut locations| {
            permutation.apply_slice_in_place(&mut component);
            permutation.apply_slice_in_place(&mut locations);

            FfiCollectionIter::from_array(locations, |iter| {
                RegistryLoader::add_components(self.registry, entity, component.as_slice(), iter)
            })
        })
    }

    pub fn get<T: Component>(&self, entity: Entity) -> Result<&T, RegistryError> {
        RegistryLoader::get_one_component(self.registry, entity, T::DESCRIPTOR.identity)
            .map(|c| c.cast::<T>())
    }
}
