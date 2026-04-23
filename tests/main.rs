use ecstasy::registry_header::query::Query;
use ecstasy::registry_header::{Component, RegistryHeader};

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

#[test]
fn creation() {
    let mut registry = RegistryHeader::new();
    let e1 = registry.new_entity((Pos { x: 0.0, y: 0.0 }, Vel { x: 1.0, y: 1.0 }));
    let e2 = registry.new_entity(Pos { x: 3.0, y: 6.0 });

    let pos1 = registry.get::<Pos>(e1).cloned();
    let vel = registry.get::<Vel>(e1).cloned();
    let pos2 = registry.get::<Pos>(e2).cloned();

    assert_eq!(pos1, Some(Pos { x: 0.0, y: 0.0 }));
    assert_eq!(vel, Some(Vel { x: 1.0, y: 1.0 }));
    assert_eq!(pos2, Some(Pos { x: 3.0, y: 6.0 }));
}

#[test]
fn addition_no_overwrite() {
    let mut registry = RegistryHeader::new();
    let e = registry.new_entity(Pos { x: 3.0, y: 6.0 });
    registry.add(e, Vel { x: 1.0, y: 1.0 }).unwrap();

    let pos = registry.get::<Pos>(e).cloned();
    let vel = registry.get::<Vel>(e).cloned();
    assert_eq!(pos, Some(Pos { x: 3.0, y: 6.0 }));
    assert_eq!(vel, Some(Vel { x: 1.0, y: 1.0 }));
}

#[test]
fn addition_with_overwrite() {
    let mut registry = RegistryHeader::new();
    let e = registry.new_entity((Pos { x: 3.0, y: 6.0 }, Vel { x: 0.0, y: 0.0 }));
    registry.add(e, Vel { x: 1.0, y: 1.0 }).unwrap();

    let pos = registry.get::<Pos>(e).cloned();
    let vel = registry.get::<Vel>(e).cloned();
    assert_eq!(pos, Some(Pos { x: 3.0, y: 6.0 }));
    assert_eq!(vel, Some(Vel { x: 1.0, y: 1.0 }));
}

#[test]
fn query() {
    let mut registry = RegistryHeader::new();
    let e1 = registry.new_entity((Pos { x: 0.0, y: 7.0 }, Vel { x: 1.0, y: 1.0 }));
    let _e2 = registry.new_entity(Pos { x: 3.0, y: 6.0 });

    let query = Query::<(&Pos, &Vel)>::new(&mut registry);
    let (pos, vel) = query.get(&registry, e1).unwrap();
    assert_eq!(*pos, Pos { x: 0.0, y: 7.0 });
    assert_eq!(*vel, Vel { x: 1.0, y: 1.0 });
}
