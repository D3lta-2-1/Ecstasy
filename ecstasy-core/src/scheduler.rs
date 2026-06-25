mod event_manager;
mod solver;

use ecstasy_ffi::{
    BorrowedResource, ConsumerOpaque, EventIndex, ProducerOpaque, RegistryOpaque,
    RegistryVtableExt, SystemContextOpaque, SystemContextVtableExt, SystemOpaque, SystemVtable,
    TypeDescriptor,
};
use reflexion::drop_location::DropLocation;

pub use crate::scheduler::event_manager::{CONSUMER_VTABLE, PRODUCER_VTABLE};

use crate::{
    boxed::Box,
    registry::Registry,
    scheduler::{event_manager::EventManager, solver::CompatibilityGraph},
};

pub struct SystemEntry {
    handle: Box<SystemOpaque>,
    vtable: &'static SystemVtable,
}

impl SystemEntry {
    pub fn borrowed_resources(&self) -> &[BorrowedResource] {
        (self.vtable.borrowed_resources)(&self.handle).into()
    }

    pub fn call(&mut self, ctx: &SystemContextOpaque) {
        (self.vtable.call)(&mut self.handle, ctx)
    }
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

    pub fn build(self) -> Scheduler {
        let graph = CompatibilityGraph::new(&self.systems);
        let system_order = graph.topological_sort();
        Scheduler {
            system: self.systems,
            system_order: system_order,
            context: SystemContext {
                registry: self.registry,
                event_manager: self.event_manager,
            },
        }
    }
}

impl ecstasy_ffi::SchedulerBuilder for SchedulerBuilder {
    #[doc = " mainly used to during system creation"]
    extern "C-unwind" fn registry(&mut self) -> &mut RegistryOpaque {
        self.registry.as_opaque_mut()
    }

    extern "C-unwind" fn find_event(&mut self, event: TypeDescriptor) -> EventIndex {
        self.event_manager.find_event(event)
    }

    extern "C-unwind" fn add_system(
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

pub struct SystemContext {
    registry: Registry,
    event_manager: EventManager,
}

/// The Scheduler one of the core part of the ECS, it does sevrals things
/// It the implementation of a game loop that most game will use
/// - store a registry
/// - propagate events
/// - run system
pub struct Scheduler {
    system: Vec<SystemEntry>,
    system_order: Vec<usize>,
    context: SystemContext,
}

impl Scheduler {
    /// advance the stored registry by exactly one tick, this function should be call at a fixed rate.
    /// In order, this will:
    /// - clear all stored events
    /// - call system in a deterministic order, and following ordering constraint induced by event manager.
    pub fn tick(&mut self) {
        self.context.event_manager.clear();
        for i in self.system_order.iter() {
            self.system[*i].call(self.context.as_opaque());
        }
    }
}

impl ecstasy_ffi::SystemContext for SystemContext {
    extern "C-unwind" fn registry(&self) -> &RegistryOpaque {
        self.registry.as_opaque()
    }

    unsafe extern "C-unwind" fn get_publisher(&self, event: EventIndex) -> &mut ProducerOpaque {
        unsafe { self.event_manager.get_unchecked_publisher(event) }
    }

    unsafe extern "C-unwind" fn get_consumer(&self, event: EventIndex) -> &ConsumerOpaque {
        unsafe { self.event_manager.get_unchecked_consumer(event) }
    }
}
