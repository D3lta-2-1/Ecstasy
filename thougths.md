# Goals of Ecstasy
With this project I aim to build an efficient ECS, mainly built for FFI interactions.
I hope to make a similar experience than Bevy, but users are free to split "Plugins" in different binary.

# Design choices

- this project is an archetype-Based ECS, like Flecs, component use entity storage,
and additional properties can be added at runtime
- The ECS should be in charge of the game loop. It will be built to run game with a fixed time loop.
This game loop shouldn't be used to render the game. This will be built with parallelism in mind.
- like bevy, the only way to access the ECS is through system. Two type of system will be available
  * Query based system, which can be run in parallel as long as they don't overlap
  * Command based system, built for entity creation/deletion. they require to acquire all archetype,
    and can't be run in parallel

# planned: 
update most integer map for perfect map, they are more costly to build, but have faster get/iter