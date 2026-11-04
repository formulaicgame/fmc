use bevy::math::I64Vec3;

use crate::coords::{ChunkPos, constants::PROTO_CHUNK_SIZE, local_block_pos::LocalBlockPos};

pub struct BlockPos(I64Vec3);

impl BlockPos {
    #[must_use]
    pub const fn new(x: i64, y: i64, z: i64) -> Self {
        Self(I64Vec3::new(x, y, z))
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
    #[expect(clippy::cast_possible_truncation)]
    pub fn chunk(self) -> ChunkPos {
        let [x, y, z] = self
            .0
            .to_array()
            .map(|v| v.div_euclid(PROTO_CHUNK_SIZE.into()) as i32);
        ChunkPos::new(x, y, z)
    }

    #[must_use]
    pub fn local(self) -> LocalBlockPos {
        let [x, y, z] = self
            .0
            .rem_euclid(I64Vec3::splat(PROTO_CHUNK_SIZE.into()))
            .as_u8vec3()
            .to_array();

        LocalBlockPos::new(x, y, z)
    }
}
