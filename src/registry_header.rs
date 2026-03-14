use crate::registry::Registry;
use crate::shared::id::{ComponentIdentity, Entity};
use reflexion::erased::DropLocation;
use reflexion::typeinfo::TypeInfoProvider;
use std::mem;

/// the registry header is the final interface between the ECS internal and "external" world.
/// it's where all clean generic methods are defined
/// each binary accessing the ECS across DLL boundaries will get a copy of all this code and data structure
/// it's the perfect place to some target local caching such as type_id <-> component identity

pub trait Component {
    const PATH: &'static str;
    const NAME: &'static str;
}

//TODO: implement bundle for any size
pub trait StaticBundle<const SIZE: usize> {
    const IDENTITIES: [ComponentIdentity; SIZE];
    fn read(self, reader: impl FnOnce([DropLocation; SIZE]) -> Entity) -> Entity;
}

impl<T: Component + TypeInfoProvider, U: Component + TypeInfoProvider> StaticBundle<2> for (T, U) {
    const IDENTITIES: [ComponentIdentity; 2] = [
        ComponentIdentity {
            path: T::PATH,
            name: T::NAME,
            type_info: T::TYPE_INFO,
        },
        ComponentIdentity {
            path: U::PATH,
            name: U::NAME,
            type_info: U::TYPE_INFO,
        },
    ];

    fn read(mut self, reader: impl FnOnce([DropLocation; 2]) -> Entity) -> Entity {
        let locations = unsafe {
            [
                DropLocation::at_hard(&mut self.0),
                DropLocation::at_hard(&mut self.1),
            ]
        };
        let e = reader(locations);
        mem::forget(self);
        e
    }
}

#[derive(Debug)]
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
        let mut component: Vec<_> = T::IDENTITIES
            .iter()
            .map(|c| self.registry.find_or_register_component(c))
            .collect();
        let mut permutation = permutation::sort(&component); //this permutation could be stored to spare some calculation...
        bundle.read(|mut locations| {
            permutation.apply_slice_in_place(&mut component);
            permutation.apply_slice_in_place(&mut locations);
            self.registry
                .create_entity(&component, locations.into_iter())
        })
    }
}
