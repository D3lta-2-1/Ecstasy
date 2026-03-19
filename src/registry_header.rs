use crate::registry::Registry;
use crate::shared::id::{ComponentIdentity, Entity};
use paste::paste;
use reflexion::erased::DropLocation;
use reflexion::typeinfo::TypeInfoProvider;
use std::mem;

/// the registry header is the final interface between the ECS internal and "external" world.
/// it's where all clean generic methods are defined
/// each binary accessing the ECS across DLL boundaries will get a copy of all this code and data structure
/// it's the perfect place to some target local caching such as type_id <-> component identity

pub trait Component: TypeInfoProvider {
    const PATH: &'static str;
    const NAME: &'static str;
    const IDENTITY: ComponentIdentity = ComponentIdentity {
        path: Self::PATH,
        name: Self::NAME,
        type_info: Self::TYPE_INFO,
    };
}

pub trait StaticBundle<const SIZE: usize> {
    const IDENTITIES: [ComponentIdentity; SIZE];
    fn read<T>(self, reader: impl FnOnce([DropLocation; SIZE]) -> T) -> T;
}

impl<T: Component> StaticBundle<1> for T {
    const IDENTITIES: [ComponentIdentity; 1] = [T::IDENTITY];

    fn read<RETURN>(mut self, reader: impl FnOnce([DropLocation; 1]) -> RETURN) -> RETURN {
        let locations = unsafe { [DropLocation::at_hard(&mut self)] };
        let r = reader(locations);
        mem::forget(self);
        r
    }
}

macro_rules! count_tts {
    () => {0usize};
    ($_head:tt $($tail:tt)*) => {1usize + count_tts!($($tail)*)};
}
macro_rules! impl_bundle {
    ($($T:tt)+) => {
        paste! {
            impl<$($T : Component,)+> StaticBundle<{ count_tts!($($T)+) }> for ($($T,)+) {
                const IDENTITIES: [ComponentIdentity; count_tts!($($T)+)] = [ $($T::IDENTITY,)+ ];
                fn read<RETURN>(self, reader: impl FnOnce([DropLocation; count_tts!($($T)+)]) -> RETURN) -> RETURN {
                    let ($(mut [<$T:lower>],)+) = self;
                    let locations = unsafe { [
                       $(DropLocation::at_hard(&mut [<$T:lower>]),)+
                    ] };
                    let r = reader(locations);
                    $(
                    mem::forget([<$T:lower>]);
                    )+
                    r
                }
            }
        }
    };
}

impl_bundle!(A);
impl_bundle!(A B);
impl_bundle!(A B C);
impl_bundle!(A B C D);
impl_bundle!(A B C D E);
impl_bundle!(A B C D E F);
impl_bundle!(A B C D E F G);
impl_bundle!(A B C D E F G H);
impl_bundle!(A B C D E F G H I);
impl_bundle!(A B C D E F G H I J);
impl_bundle!(A B C D E F G H I J K);
impl_bundle!(A B C D E F G H I J K L);

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
        let mut component: [Entity; SIZE] =
            std::array::from_fn(|i| self.registry.find_or_register_component(&T::IDENTITIES[i]));
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
    ) -> Option<()> {
        let mut component: [Entity; SIZE] =
            std::array::from_fn(|i| self.registry.find_or_register_component(&T::IDENTITIES[i]));

        let mut permutation = permutation::sort(&component); //this permutation could be stored to spare some calculation...
        bundle.read(|mut locations| {
            permutation.apply_slice_in_place(&mut component);
            permutation.apply_slice_in_place(&mut locations);
            self.registry
                .add_components(entity, &component, locations.into_iter())
        })
    }

    pub fn get_single<T: Component>(&self, entity: Entity) -> Option<&T> {
        let component = self.registry.find_component(&T::IDENTITY)?;
        self.registry
            .get_one_component(entity, component)
            .map(|c| c.cast::<T>())
    }
}
