use ecstasy::registry_header::{Component, RegistryHeader};

struct Pos{
    x: f32,
    y: f32
}

impl Component for Pos {
    const PATH: &'static str = "test";
    const NAME: &'static str = "pos";
}

struct Vel{
    x: f32,
    y: f32
}

impl Component for Vel {
    const PATH: &'static str = "test";
    const NAME: &'static str = "vel";
}

#[test]
fn test1() {
    let mut registry = RegistryHeader::new();
    let e = registry.new_entity((Pos{x: 0.0, y:0.0}, Vel{x: 0.0, y:0.0}));
    println!("entity: {:?}", e);
    println!("{:?}", registry);
}
