use bevy::{
    math::{DVec3, UVec2, Vec2, Vec3},
    prelude::*,
};
use fmc_protocol_derive::ClientBound;
use serde::{Deserialize, Serialize};

#[derive(ClientBound, Message, Serialize, Deserialize, Debug, Clone)]
pub struct ParticleEffect {
    /// Spawn location
    pub position: DVec3,
    /// Maximum offset a particle can be spawned at
    pub spawn_offset: Vec3,
    /// Randomized min/max length of mesh quad sides
    pub size_range: Vec2,
    /// Randomized magnitude of initial velocity. If the spawn_offset is not Vec3::ZERO, this will
    /// yield a velocity vector along the vector between the spawn point and the origin. Otherwise
    /// its direction is random.
    pub velocity: Vec2,
    /// Path to the texture, relative to "textures/"
    pub texture: String,
    /// Tint
    pub color: Vec4,
    /// Randomized min/max lifetime for each particle
    pub lifetime: Vec2,
    /// For each particle spawned, render it with a smaller section of the texture. Measured in
    /// 1/16 units, first element is minimum amount, last is max
    pub random_uv: Option<UVec2>,
    /// How many particles should be spawned in total
    pub count: u32,
    /// If physics collision should be enabled
    pub collision: bool,
    /// If physics friction should be enabled
    pub friction: Vec3,
    /// Gravity applied to each particle
    pub gravity: Vec3,
}
