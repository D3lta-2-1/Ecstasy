pub mod bundle;

use crate::registry::Registry;
pub use crate::registry_header::bundle::{Component, StaticBundle};
use crate::shared::id::Entity;

/// the registry header is the final interface between the ECS internal and "external" world.
/// it's where all clean generic methods are defined
/// each binary accessing the ECS across DLL boundaries will get a copy of all this code and data structure
/// it's the perfect place to some target local caching such as type_id <-> component identity
pub struct RegistryHeader {
    registry: Registry,
}

impl RegistryHeader {
    pub fn new() -> Self {
        Self {
            registry: Registry::new(),
        }
    }

    pub fn new_entity<const SIZE: usize, T: StaticBundle<SIZE>>(&mut self, bundle: T) -> Entity {
        let mut component: [Entity; SIZE] =
            std::array::from_fn(|i| self.registry.find_or_register_component(&T::DESCRIPTORS[i]));
        let mut permutation = permutation::sort(&component); //this permutation could be stored to spare some calculation...
        bundle.read(|mut locations| {
            permutation.apply_slice_in_place(&mut component);
            permutation.apply_slice_in_place(&mut locations);
            self.registry
                .create_entity(&component, locations.into_iter())
        })
    }

    pub fn add<const SIZE: usize, T: StaticBundle<SIZE>>(
        &mut self,
        entity: Entity,
        bundle: T,
    ) -> Result<(), ()> {
        let mut component: [Entity; SIZE] =
            std::array::from_fn(|i| self.registry.find_or_register_component(&T::DESCRIPTORS[i]));

        let mut permutation = permutation::sort(&component); //this permutation could be stored to spare some calculation...
        bundle.read(|mut locations| {
            permutation.apply_slice_in_place(&mut component);
            permutation.apply_slice_in_place(&mut locations);
            self.registry
                .add_components(entity, &component, locations.into_iter())
        })
    }

    pub fn get_single<T: Component>(&self, entity: Entity) -> Option<&T> {
        self.registry
            .get_one_component(entity, T::DESCRIPTOR.identity)
            .map(|c| c.cast::<T>())
    }
}
