use std::mem;

use reflexion::drop_location::DropLocation;

use crate::{
    RegistryHeader,
    loader::SchedulerBuilderLoader,
    system::{IntoSystem, SystemParam},
};
use ecstasy_ffi::{SchedulerBuilderOpaque, SystemVtableExt};

pub struct SchedulerBuilder<'a> {
    inner: &'a mut SchedulerBuilderOpaque,
}

impl<'a> SchedulerBuilder<'a> {
    pub fn new(opaque: &'a mut SchedulerBuilderOpaque) -> Self {
        Self { inner: opaque }
    }

    pub fn registry(&mut self) -> RegistryHeader<'_> {
        RegistryHeader::new(SchedulerBuilderLoader::registry(self.inner))
    }

    pub fn add_systeme<System: IntoSystem<Params>, Params: SystemParam>(&mut self, system: System) {
        let mut system = system.into_system(self.inner);
        unsafe {
            let location = DropLocation::at_hard(&mut system);
            SchedulerBuilderLoader::add_system(self.inner, location, &System::System::VTABLE);
            mem::forget(system);
        }
    }
}
