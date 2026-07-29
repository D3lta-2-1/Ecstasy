use paste::paste;
use reflexion::{drop_location::DropLocation, ffi_slice::FfiSlice, typeinfo::TypeInfoProvider};
use std::mem;

use ecstasy_ffi::{ComponentDescriptor, TypeIdentity};

pub trait Component: TypeInfoProvider {
    const PATH: &'static str;
    const NAME: &'static str;
    const VERSIONED: bool = false;
    const DESCRIPTOR: ComponentDescriptor = ComponentDescriptor {
        identity: TypeIdentity {
            path: FfiSlice::from_str(Self::PATH),
            name: FfiSlice::from_str(Self::NAME),
        },
        type_info: Self::TYPE_INFO,
        versioned: Self::VERSIONED,
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
