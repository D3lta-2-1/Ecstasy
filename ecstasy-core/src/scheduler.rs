mod event_manager;

use ecstasy_ffi::{
    EventIndex, PublisherHandle, RegistryOpaque, RegistryVtableExt, SystemOpaque, SystemVtable,
    TypeDescriptor,
};
use reflexion::drop_location::DropLocation;

use crate::{boxed::Box, registry::Registry, scheduler::event_manager::EventManager};

struct SystemEntry {
    handle: Box<SystemOpaque>,
    vtable: &'static SystemVtable,
}

pub struct SchedulerBuilder {
    systems: Vec<SystemEntry>, //Ordered Version of the System behavior
    registry: Registry,
    event_manager: EventManager,
}

impl SchedulerBuilder {
    pub fn new() -> Self {
        Self {
            systems: Vec::new(),
            registry: Registry::new(),
            event_manager: EventManager::default(),
        }
    }
}

impl ecstasy_ffi::SchedulerBuilder for SchedulerBuilder {
    #[doc = " mainly used to during system creation"]
    extern "C" fn registry(&mut self) -> &mut RegistryOpaque {
        self.registry.as_opaque_mut()
    }

    extern "C" fn find_event(&mut self, event: TypeDescriptor) -> EventIndex {
        self.event_manager.find_event(event)
    }

    extern "C" fn add_system(
        &mut self,
        system: DropLocation<SystemOpaque>,
        vtable: &'static SystemVtable,
    ) {
        let system = SystemEntry {
            handle: Box::new(system),
            vtable,
        };
        self.systems.push(system);
    }
}

/// The Scheduler one of the core part of the ECS, it does sevrals things
/// It the implementation of a game loop that most game will use
/// - store a registry
/// - propagate events
/// - run system
pub struct Scheduler {
    systems: Vec<SystemEntry>, //Ordered Version of the System behavior
    registry: Registry,
    event_manager: EventManager,
}

impl Scheduler {}

impl ecstasy_ffi::SystemContext for Scheduler {
    extern "C" fn registry(&self) -> &RegistryOpaque {
        self.registry.as_opaque()
    }

    unsafe extern "C" fn get_publisher(&self, event: EventIndex) -> PublisherHandle<'_> {
        unsafe { self.event_manager.get_unchecked_publisher(event) }
    }
}
