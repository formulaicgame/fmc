use fmc_vanilla::{
    // The 'fmc' module is the base layer of the game. It takes care of things like chunk
    // management, networking, physics, etc... Things that are specific to the game are found in
    // the 'fmc_vanilla' module, mobs, items and the like. Check out the docs to see what is
    // available.
    fmc::{
        networking::Server,
        players::{Camera, Player},
        protocol::messages,
    },
    // The prelude includes many of Bevy's often used primitives.
    // If you don't know Bevy yet, these are some useful resources:
    //
    // https://bevyengine.org/learn/quick-start/getting-started/ecs/ (ecs section)
    // https://bevy-cheatbook.github.io/programming.html (Chapters 14 and 5)
    // Ignore everything that has to do with rendering, bevy is a larger framework for game
    // development, but we only use the ecs part of it.
    //
    // https://dev-docs.bevyengine.org/bevy_ecs/index.html
    // Bevy's docs, you will use these a lot.
    //
    // https://github.com/bevyengine/bevy/tree/main/examples#ecs-entity-component-system
    // Exhaustive examples of available features
    prelude::*,
};

// Your mod must expose a 'Mod' struct that implements Plugin to be recognized as a mod.
pub struct Mod;
impl Plugin for Mod {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, barf);
    }
}

// Spawns a particle effect in front of all players
fn barf(server: Res<Server>, players: Query<(Entity, &Transform, &Camera), With<Player>>) {
    for (player_entity, transform, camera) in players.iter() {
        let camera_position = transform.translation + camera.translation;
        let forward = camera.forward();

        let position = camera_position + forward;
        server.send_one(player_entity, messages::ParticleEffect::Explosion {
            position,
            spawn_offset: Vec3::ZERO,
            size_range: (0.5, 1.0),
            min_velocity: Vec3::NEG_ONE,
            max_velocity: Vec3::ONE,
            texture: Some(String::from("blocks/grass_top.png")),
            color: None,
            lifetime: (1.0, 2.0),
            count: 5,
        });
    }
}
