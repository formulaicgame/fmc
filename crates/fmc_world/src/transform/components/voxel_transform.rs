use bevy::prelude::*;

use crate::{
    coords::{BlockPos, ChunkPos, constants::PROTO_CHUNK_SIZE},
    transform::GlobalVoxelTransform,
};

#[derive(Component, Clone, Debug, PartialEq)]
#[require(Transform, GlobalVoxelTransform)]
pub struct VoxelTransform {
    pub chunk: ChunkPos,
    pub local: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}

impl VoxelTransform {
    #[must_use]
    pub fn from_chunk_local(chunk: ChunkPos, local: Vec3) -> Self {
        let mut t = Self {
            chunk,
            local,
            ..Default::default()
        };

        t.normalize();
        t
    }

    pub fn normalize(&mut self) {
        let carry = (self.local / Vec3::splat(PROTO_CHUNK_SIZE.into())).floor();

        self.chunk += carry.as_ivec3();
        self.local -= carry * Vec3::splat(PROTO_CHUNK_SIZE.into());

        for i in 0..3 {
            if self.local[i] >= PROTO_CHUNK_SIZE.into() {
                self.local[i] = 0.0;
                self.chunk[i] += 1;
            }
        }
    }

    #[must_use]
    pub fn to_render(&self, origin: ChunkPos) -> Transform {
        Transform {
            translation: (self.chunk.as_i64vec3() - origin.as_i64vec3()).as_vec3()
                * Vec3::splat(PROTO_CHUNK_SIZE.into())
                + self.local,
            rotation: self.rotation,
            scale: self.scale,
        }
    }

    #[must_use]
    pub fn block_pos(&self) -> BlockPos {
        BlockPos::from_i64vec3(
            self.chunk.as_i64vec3() * PROTO_CHUNK_SIZE as i64 + self.local.floor().as_i64vec3(),
        )
    }

    #[must_use]
    pub fn offset_from(&self, other: &Self) -> Vec3 {
        (self.chunk.as_ivec3() - other.chunk.as_ivec3())
            .abs()
            .as_vec3()
            + (self.local - other.local).abs()
    }

    pub fn translated_by(&self, translation: Vec3) -> Self {
        Self::from_chunk_local(self.chunk, self.local + translation)
    }
}

impl Default for VoxelTransform {
    fn default() -> Self {
        Self {
            chunk: ChunkPos::default(),
            local: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        }
    }
}
