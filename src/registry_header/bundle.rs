use crate::shared::id::{ComponentDescriptor, ComponentIdentity};
use paste::paste;
use reflexion::erased::DropLocation;
use reflexion::typeinfo::TypeInfoProvider;
use std::mem;

pub trait Component: TypeInfoProvider {
    const PATH: &'static str;
    const NAME: &'static str;
    const DESCRIPTOR: ComponentDescriptor = ComponentDescriptor {
        identity: ComponentIdentity {
            path: Self::PATH,
            name: Self::NAME,
        },
        type_info: Self::TYPE_INFO,
    };
}

pub trait StaticBundle<const SIZE: usize> {
    const DESCRIPTORS: [ComponentDescriptor; SIZE];
    fn read<T>(self, reader: impl FnOnce([DropLocation; SIZE]) -> T) -> T;
}

impl<T: Component> StaticBundle<1> for T {
    const DESCRIPTORS: [ComponentDescriptor; 1] = [T::DESCRIPTOR];

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
                const DESCRIPTORS: [ComponentDescriptor; count_tts!($($T)+)] = [ $($T::DESCRIPTOR,)+ ];
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
