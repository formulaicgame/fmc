use std::ops::{Add, AddAssign};

use bevy::prelude::*;

use crate::position::{BlockPos, components::ChunkPos};

#[derive(Component, Default)]
pub struct WorldPos {
    pub chunk: ChunkPos,
    pub local: Vec3,
}

impl WorldPos {
    #[must_use]
    pub fn new(chunk: ChunkPos, local: Vec3) -> Self {
        let mut p = Self { chunk, local };
        p.normalize();

        p
    }

    pub fn normalize(&mut self) {
        let shift = (self.local / 16.0).floor();
        self.chunk.0 += shift.as_i64vec3();
        self.local -= shift * 16.0;

        for i in 0..3 {
            if self.local[i] >= 16.0 {
                self.local[i] = 0.0;
                self.chunk.0[i] += 1;
            }
        }
    }

    #[must_use]
    pub fn relative_to(&self, origin: ChunkPos) -> Vec3 {
        (self.chunk.0 - origin.0).as_vec3() * 16.0 + self.local
    }

    #[must_use]
    pub fn block(&self) -> BlockPos {
        BlockPos::from_i64vec3(self.chunk.0 * 16 + self.local.floor().as_i64vec3())
    }
}

impl Add<Vec3> for WorldPos {
    type Output = Self;

    fn add(self, rhs: Vec3) -> Self::Output {
        Self::new(self.chunk, self.local + rhs)
    }
}

impl AddAssign<Vec3> for WorldPos {
    fn add_assign(&mut self, rhs: Vec3) {
        *self = Self::new(self.chunk, self.local + rhs);
    }
}
