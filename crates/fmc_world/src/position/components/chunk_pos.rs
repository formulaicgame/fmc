use bevy::{math::I64Vec3, prelude::*};

/// Represents a chunk position in the world.
#[derive(Debug, Default, PartialEq, Eq, Hash, Component, Clone, Copy)]
pub struct ChunkPos(pub(crate) I64Vec3);

impl ChunkPos {
    #[must_use]
    pub const fn new(x: i64, y: i64, z: i64) -> Self {
        Self(I64Vec3::new(x, y, z))
    }

    #[must_use]
    pub const fn from_i64vec3(v: I64Vec3) -> Self {
        Self(v)
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
}
