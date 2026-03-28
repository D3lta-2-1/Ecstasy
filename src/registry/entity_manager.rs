use crate::registry::{ArchetypeIndex, EntityIndex, MovedEntity};
use crate::shared::id::Entity;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EntityLocation {
    pub archetype_index: ArchetypeIndex,
    pub entity_index: EntityIndex,
}

#[derive(Default)]
/// Store where entity are located in the registry, and manage entity id allocation
pub struct EntityManager {
    short_living_entity: Vec<Option<(Entity, EntityLocation)>>, //no need to go for an u8 since padding will probably fill up the gap...
    long_living_entity: Vec<EntityLocation>,                    // these entities can't be deleted
    free_entity: Vec<Entity>, //both contain the last generation and the free spot
}

impl EntityManager {
    pub fn get(&self, entity: Entity) -> Option<EntityLocation> {
        match entity.generation() {
            0 => None,
            1 => self
                .long_living_entity
                .get(entity.index() as usize)
                .cloned(),
            _gen @ 2..=u8::MAX => {
                let (local_entity, location) = self
                    .short_living_entity
                    .get(entity.index() as usize)
                    .cloned()??;
                (entity == local_entity).then_some(location)
            }
        }
    }

    pub fn update_location(
        &mut self,
        MovedEntity {
            entity,
            new_location,
        }: MovedEntity,
    ) {
        let (id, location) = self
            .short_living_entity
            .get_mut(entity.index() as usize)
            .map(|o| o.as_mut())
            .flatten()
            .expect("this entity doesn't exist");
        assert_eq!(*id, entity, "this entity doesn't exist anymore");
        *location = new_location;
    }

    pub fn allocate(&mut self, builder: impl FnOnce(Entity) -> EntityLocation) -> Entity {
        // if there is a free spot
        let recycled = self.free_entity.pop().map(|id| {
            let generation = id.generation().wrapping_add(1).min(2);
            Entity::new(generation, id.index())
        });
        // else append to the end
        let entity = recycled.unwrap_or_else(|| {
            let entity = Entity::new(3, self.short_living_entity.len() as u32);
            self.short_living_entity.push(None);
            entity
        });
        let location = builder(entity);
        let slot = &mut self.short_living_entity[entity.index() as usize];
        assert_eq!(*slot, None);
        *slot = Some((entity, location));
        entity
    }

    pub fn allocate_permanent(&mut self, builder: impl FnOnce(Entity) -> EntityLocation) -> Entity {
        let id = self.long_living_entity.len();
        let entity = Entity::new(1, id as u32);
        let location = builder(entity);

        self.long_living_entity.push(location);
        entity
    }

    /// mark this ID a free for futur uses, return the previous location if any
    pub fn free(&mut self, entity: Entity) -> Option<EntityLocation> {
        assert_ne!(entity.generation(), 1, "this entity can't be deleted");
        let value = self
            .short_living_entity
            .get_mut(entity.generation() as usize)?;

        let (id, location) = value.clone()?;
        if id != entity {
            return None;
        };
        self.free_entity.push(entity);
        *value = None;
        Some(location)
    }
}
