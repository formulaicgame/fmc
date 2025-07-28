use bevy::{
    math::{DVec3, Vec3},
    prelude::*,
};
use fmc_protocol_derive::ClientBound;
use serde::{Deserialize, Serialize};

#[derive(ClientBound, Event, Serialize, Deserialize, Debug, Clone)]
pub enum ParticleEffect {
    Explosion {
        /// Spawn location
        position: DVec3,
        /// Maximum offset a particle can be spawned at
        spawn_offset: Vec3,
        /// Min and max length of mesh quad
        size_range: (f32, f32),
        /// Minimum initial velocity
        min_velocity: Vec3,
        /// Maximum initial velocity
        max_velocity: Vec3,
        /// Path to texture, relative to /textures/
        texture: Option<String>,
        /// Hex encoded rgba
        color: Option<String>,
        /// Min to max lifetime of each particle
        lifetime: (f32, f32),
        /// How many particles should be spawned
        count: u32,
    },
}
