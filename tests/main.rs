use ecstasy::{
    Component, Query, RegistryHeader,
    loader::{Ecstasy, EcstasyContext},
    query::QueryState,
};
use ecstasy_ffi::{
    QuerySetVtableExt, RegistryVtableExt, SchedulerBuilderVtableExt, SystemContextVtableExt,
};

use ecstasy_core::{
    registry::{Registry, query::QuerySet},
    scheduler::{Scheduler, SchedulerBuilder},
};

#[derive(Debug, Copy, Clone, PartialEq)]
struct Pos {
    x: f32,
    y: f32,
}

impl Component for Pos {
    const PATH: &'static str = "test";
    const NAME: &'static str = "pos";
}

#[derive(Debug, Copy, Clone, PartialEq)]
struct Vel {
    x: f32,
    y: f32,
}

impl Component for Vel {
    const PATH: &'static str = "test";
    const NAME: &'static str = "vel";
}

const CONTEXT: EcstasyContext = EcstasyContext {
    registry: &Registry::VTABLE,
    query_set: &QuerySet::VTABLE,
    scheduler_builder: &SchedulerBuilder::VTABLE,
    system_context: &Scheduler::VTABLE,
};

#[test]
fn creation() {
    let _ = Ecstasy::load(CONTEXT);

    let mut registry_impl = Registry::new();
    let mut registry = RegistryHeader::new(registry_impl.as_opaque_mut());
    let e1 = registry.new_entity((Pos { x: 0.0, y: 0.0 }, Vel { x: 1.0, y: 1.0 }));
    let e2 = registry.new_entity(Pos { x: 3.0, y: 6.0 });

    let pos1 = registry.get::<Pos>(e1).cloned();
    let vel = registry.get::<Vel>(e1).cloned();
    let pos2 = registry.get::<Pos>(e2).cloned();

    assert_eq!(pos1, Ok(Pos { x: 0.0, y: 0.0 }));
    assert_eq!(vel, Ok(Vel { x: 1.0, y: 1.0 }));
    assert_eq!(pos2, Ok(Pos { x: 3.0, y: 6.0 }));
}

#[test]
fn addition_no_overwrite() {
    let _ = Ecstasy::load(CONTEXT);

    let mut registry_impl = Registry::new();
    let mut registry = RegistryHeader::new(registry_impl.as_opaque_mut());
    let e = registry.new_entity(Pos { x: 3.0, y: 6.0 });
    registry.add(e, Vel { x: 1.0, y: 1.0 }).unwrap();

    let pos = registry.get::<Pos>(e).cloned();
    let vel = registry.get::<Vel>(e).cloned();
    assert_eq!(pos, Ok(Pos { x: 3.0, y: 6.0 }));
    assert_eq!(vel, Ok(Vel { x: 1.0, y: 1.0 }));
}

#[test]
fn addition_with_overwrite() {
    let _ = Ecstasy::load(CONTEXT);

    let mut registry_impl = Registry::new();
    let mut registry = RegistryHeader::new(registry_impl.as_opaque_mut());
    let e = registry.new_entity((Pos { x: 3.0, y: 6.0 }, Vel { x: 0.0, y: 0.0 }));
    registry.add(e, Vel { x: 1.0, y: 1.0 }).unwrap();

    let pos = registry.get::<Pos>(e).cloned();
    let vel = registry.get::<Vel>(e).cloned();
    assert_eq!(pos, Ok(Pos { x: 3.0, y: 6.0 }));
    assert_eq!(vel, Ok(Vel { x: 1.0, y: 1.0 }));
}

#[test]
fn query() {
    let _ = Ecstasy::load(CONTEXT);

    let mut registry_impl = Registry::new();
    let mut registry = RegistryHeader::new(registry_impl.as_opaque_mut());
    let e1 = registry.new_entity((Pos { x: 0.0, y: 7.0 }, Vel { x: 1.0, y: 1.0 }));
    let _e2 = registry.new_entity(Pos { x: 3.0, y: 6.0 });
    let query = QueryState::<(&Pos, &Vel)>::new(registry.registry());
    let (pos, vel) = query.get(registry.registry(), e1).unwrap();
    assert_eq!(*pos, Pos { x: 0.0, y: 7.0 });
    assert_eq!(*vel, Vel { x: 1.0, y: 1.0 });
}

#[test]
fn test_system() {
    let _ = Ecstasy::load(CONTEXT);
    let mut builder_impl = ecstasy_core::scheduler::SchedulerBuilder::new();
    let mut builder = ecstasy::SchedulerBuilder::new(builder_impl.as_opaque_mut());

    let mut registry = builder.registry();
    let e1 = registry.new_entity((Pos { x: 0.0, y: 7.0 }, Vel { x: 1.0, y: 1.0 }));
    let _e2 = registry.new_entity(Pos { x: 3.0, y: 6.0 });
    let _ = registry;

    let system = move |query: Query<(&Pos, &Vel)>| {
        let (pos, vel) = query.get(e1).unwrap();
        assert_eq!(*pos, Pos { x: 0.0, y: 7.0 });
        assert_eq!(*vel, Vel { x: 1.0, y: 1.0 });
    };

    builder.add_systeme(system);
}
