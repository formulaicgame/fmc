// This is a mod for fmc_173, replace fmc_173 with the game you want to mod in Cargo.toml
// Use the game's docs to discover functionality you can modify.

use fmc_173::{
    // The 'fmc' module is the core of the game. It takes care of chunk management,
    // networking, physics, etc... Things that are specific to the game are found in
    // the 'fmc_173' module, mobs, items and the like.
    fmc::{
        networking::Server,
        particle_effects::ParticleEffects,
        players::{Camera, Player},
        protocol::messages,
    },
    // The prelude includes many of Bevy's often used primitives.
    // If you don't know Bevy yet, these are some useful resources:
    //
    // https://bevyengine.org/learn/quick-start/getting-started/ecs/ (ecs section)
    // https://bevy-cheatbook.github.io/programming.html (Chapters 14 and 5)
    // Ignore everything else, bevy is a larger framework for game
    // development, but we only use the ecs part of it.
    //
    // https://docs.rs/bevy/latest/bevy/
    // Bevy's docs
    //
    // https://github.com/bevyengine/bevy/tree/main/examples#ecs-entity-component-system
    // Exhaustive examples of available ecs features
    prelude::*,
};

// Your mod must expose a struct named `Mod` that implements `bevy::app::Plugin` to be recognized as a mod.
pub struct Mod;
impl Plugin for Mod {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, barf);
    }
}

// Spawns a particle effect in front of all players
fn barf(
    server: Res<Server>,
    particle_effects: Res<ParticleEffects>,
    players: Query<(Entity, &Transform, &Camera), With<Player>>,
) {
    for (player_entity, transform, camera) in players.iter() {
        let camera_position = transform.translation + camera.translation;
        let forward = camera.forward();

        let position = camera_position + forward;
        server.send_one(
            player_entity,
            messages::ParticleEffect {
                id: particle_effects.get_id("barf").unwrap(),
                position,
                rotation: Quat::IDENTITY,
                texture: String::from("blocks/grass_top.png"),
                color: Vec4::ONE,
            },
        );
    }
}
