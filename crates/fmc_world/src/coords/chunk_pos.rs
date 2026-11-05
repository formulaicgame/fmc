use std::ops::{Add, AddAssign, Index, IndexMut};

use bevy::math::{DVec3, I64Vec3, IVec3};

use crate::coords::constants::PROTO_CHUNK_SIZE;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChunkPos(IVec3);

impl ChunkPos {
    #[must_use]
    pub const fn new(x: i32, y: i32, z: i32) -> Self {
        Self(IVec3::new(x, y, z))
    }

    #[must_use]
    pub const fn x(self) -> i32 {
        self.0.x
    }

    #[must_use]
    pub const fn y(self) -> i32 {
        self.0.y
    }

    #[must_use]
    pub const fn z(self) -> i32 {
        self.0.z
    }

    #[must_use]
    pub fn as_i64vec3(self) -> I64Vec3 {
        self.0.as_i64vec3()
    }

    #[must_use]
    pub fn as_ivec3(self) -> IVec3 {
        self.0
    }

    #[must_use]
    pub fn corner(self) -> DVec3 {
        self.0.as_dvec3() * f64::from(PROTO_CHUNK_SIZE)
    }
}

impl Default for ChunkPos {
    fn default() -> Self {
        Self(IVec3::ZERO)
    }
}

impl Add<IVec3> for ChunkPos {
    type Output = Self;

    fn add(self, rhs: IVec3) -> Self::Output {
        Self(self.0 + rhs)
    }
}

impl AddAssign<IVec3> for ChunkPos {
    fn add_assign(&mut self, rhs: IVec3) {
        self.0 += rhs;
    }
}

impl IndexMut<usize> for ChunkPos {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        match index {
            0 => &mut self.0.x,
            1 => &mut self.0.y,
            2 => &mut self.0.z,
            _ => unreachable!(),
        }
    }
}

impl Index<usize> for ChunkPos {
    type Output = i32;

    fn index(&self, index: usize) -> &Self::Output {
        match index {
            0 => &self.0.x,
            1 => &self.0.y,
            2 => &self.0.z,
            _ => unreachable!(),
        }
    }
}
