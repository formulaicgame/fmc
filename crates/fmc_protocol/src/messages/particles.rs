use bevy::{math::DVec3, prelude::*};
use fmc_protocol_derive::ClientBound;
use serde::{Deserialize, Serialize};

#[derive(ClientBound, Message, Serialize, Deserialize, Debug, Clone)]
pub struct ParticleEffect {
    /// Asset id of the particle effect to spawn
    pub id: u32,
    /// World position the effect is spawned at
    pub position: DVec3,
    /// Rotation applied to the effect's local-space positions and velocities
    pub rotation: Quat,
    /// Path to the texture, relative to "textures/"
    pub texture: String,
    /// Tint
    pub color: Vec4,
}

/// Spawn a particle spawner that repeatedly spawns a particle effect.
#[derive(ClientBound, Message, Serialize, Deserialize, Debug, Clone)]
pub struct ParticleSpawner {
    /// Id used to reference the spawner. If the same id is sent twice, the old spawner will be
    /// replaced.
    pub id: u32,
    /// World position the effect is spawned at
    pub position: DVec3,
    /// Rotation applied to the effect's local-space positions and velocities
    pub rotation: Quat,
    /// Asset id of the particle effect to spawn
    pub effect: u32,
    /// Path to the texture, relative to "textures/"
    pub texture: String,
    /// Tint
    pub color: Vec4,
    /// Minimum and maximum time in seconds between each time the effect is spawned
    pub interval: [f32; 2],
}

/// Delete an existing particle spawner.
#[derive(ClientBound, Message, Serialize, Deserialize, Debug, Clone)]
pub struct DeleteParticleSpawner {
    /// Id of the particle spawner
    pub id: u32,
}
