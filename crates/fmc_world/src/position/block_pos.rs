use bevy::{math::I64Vec3, prelude::*};

use crate::position::{LocalBlockPos, components::ChunkPos};

/// Represents a block position in the world.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlockPos(I64Vec3);

impl BlockPos {
    #[must_use]
    pub const fn new(x: i64, y: i64, z: i64) -> Self {
        Self(I64Vec3::new(x, y, z))
    }

    #[must_use]
    pub const fn from_i64vec3(v: I64Vec3) -> Self {
        Self(v)
    }

    #[must_use]
    pub fn from_vec3(v: Vec3) -> Self {
        Self(v.floor().as_i64vec3())
    }

    #[must_use]
    pub const fn x(self) -> i64 {
        self.0.x
    }

    #[must_use]
    pub const fn y(self) -> i64 {
        self.0.y
    }

    #[must_use]
    pub const fn z(self) -> i64 {
        self.0.z
    }

    #[must_use]
    pub fn chunk(self) -> ChunkPos {
        ChunkPos::from_i64vec3(self.0.div_euclid(I64Vec3::splat(16)))
    }

    #[must_use]
    pub fn local(self) -> LocalBlockPos {
        LocalBlockPos::from_u8vec3(self.0.rem_euclid(I64Vec3::splat(16)).as_u8vec3())
    }
}
