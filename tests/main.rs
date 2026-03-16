use ecstasy::registry_header::{Component, RegistryHeader};

#[derive(Debug, Copy, Clone)]
struct Pos {
    x: f32,
    y: f32,
}

impl Component for Pos {
    const PATH: &'static str = "test";
    const NAME: &'static str = "pos";
}

#[derive(Debug, Copy, Clone)]
struct Vel {
    x: f32,
    y: f32,
}

impl Component for Vel {
    const PATH: &'static str = "test";
    const NAME: &'static str = "vel";
}

#[test]
fn test1() {
    let mut registry = RegistryHeader::new();
    let e1 = registry.new_entity((Pos { x: 0.0, y: 0.0 }, Vel { x: 1.0, y: 1.0 }));
    let e2 = registry.new_entity(Pos { x: 3.0, y: 6.0 });
    println!("{:?}", registry);
    let pos1 = registry.get_single::<Pos>(e1).cloned();
    let vel = registry.get_single::<Vel>(e1).cloned();
    let pos2 = registry.get_single::<Pos>(e2).cloned();
    println!("pos: {:?}, vel: {:?}", pos1, vel);
    println!("pos: {:?}", pos2);
}
