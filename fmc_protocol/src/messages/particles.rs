use bevy::{math::DVec3, prelude::*};
use fmc_protocol_derive::ClientBound;
use serde::{Deserialize, Serialize};

#[derive(ClientBound, Message, Serialize, Deserialize, Debug, Clone)]
pub struct ParticleEffect {
    /// Id of the particle effect asset to spawn
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
