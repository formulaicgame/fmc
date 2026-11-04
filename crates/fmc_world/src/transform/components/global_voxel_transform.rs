use bevy::prelude::*;

use crate::coords::ChunkPos;

#[derive(Component, Clone, Debug, PartialEq)]
pub struct GlobalVoxelTransform {
    pub chunk: ChunkPos,
    pub local: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}

impl Default for GlobalVoxelTransform {
    fn default() -> Self {
        Self {
            chunk: ChunkPos::default(),
            local: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        }
    }
}
