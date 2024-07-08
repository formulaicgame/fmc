use std::collections::HashMap;
use std::ops::{Index, IndexMut};

use bevy::prelude::*;

use crate::utils;
use crate::world::blocks::{BlockId, BlockState};

#[derive(Component)]
pub struct ChunkMarker;

/// There are two kinds of chunks.
/// Uniform(air, solid stone, etc) chunks:
///     entity = None
///     blocks = Vec::with_capacity(1), contains type of block
/// Chunks with blocks:
///     entity = Some
///     blocks = Vec::with_capacity(Chunk::SIZE^3)
#[derive(Clone)]
pub struct Chunk {
    // Entity in the ECS. Stores mesh, None if the chunk shouldn't have one.
    pub entity: Option<Entity>,
    /// XXX: Notice that the coordinates align with the rendering world, the z axis extends
    /// out of the screen. 0,0,0 is the bottom left FAR corner. Not bottom left NEAR.
    /// A Chunk::SIZE^3 array containing all the blocks in the chunk.
    /// Indexed by x*Chunk::SIZE^2 + z*CHUNK_SIZE + y
    blocks: Vec<BlockId>,
    /// Optional block state
    block_state: HashMap<usize, BlockState>,
}

impl Chunk {
    pub const SIZE: usize = 16;

    /// Build a normal chunk
    pub fn new(
        entity: Entity,
        blocks: Vec<BlockId>,
        block_state: HashMap<usize, BlockState>,
    ) -> Self {
        return Self {
            entity: Some(entity),
            blocks,
            block_state,
        };
    }

    /// Create a new chunk of only air blocks; to be filled after creation.
    pub fn new_air(blocks: Vec<BlockId>, block_state: HashMap<usize, BlockState>) -> Self {
        assert!(blocks.len() == 1);

        return Self {
            entity: None,
            block_state,
            blocks,
        };
    }

    pub fn convert_uniform_to_full(&mut self) {
        if !self.is_uniform() {
            panic!("Tried to convert a non uniform chunk");
        }
        let block = self.blocks[0];
        self.blocks = vec![block; Chunk::SIZE.pow(3)]
    }

    pub fn is_uniform(&self) -> bool {
        return self.blocks.len() == 1;
    }

    pub fn set_block_state(&mut self, block_index: usize, state: BlockState) {
        self.block_state.insert(block_index, state);
    }

    pub fn remove_block_state(&mut self, block_index: &usize) {
        self.block_state.remove(&block_index);
    }

    pub fn get_block_state(&self, x: usize, y: usize, z: usize) -> Option<BlockState> {
        let index = x << 8 | z << 4 | y;
        return self.block_state.get(&index).copied();
    }
}

impl Index<usize> for Chunk {
    type Output = BlockId;

    fn index(&self, idx: usize) -> &Self::Output {
        if self.is_uniform() {
            return &self.blocks[0];
        } else {
            return &self.blocks[idx];
        }
    }
}

impl IndexMut<usize> for Chunk {
    fn index_mut(&mut self, idx: usize) -> &mut Self::Output {
        if self.is_uniform() {
            return &mut self.blocks[0];
        } else {
            return &mut self.blocks[idx];
        }
    }
}

impl Index<[usize; 3]> for Chunk {
    type Output = BlockId;

    fn index(&self, idx: [usize; 3]) -> &Self::Output {
        if self.is_uniform() {
            return &self.blocks[0];
        } else {
            return &self.blocks[idx[0] * Chunk::SIZE.pow(2) + idx[2] * Chunk::SIZE + idx[1]];
        }
    }
}

impl IndexMut<[usize; 3]> for Chunk {
    fn index_mut(&mut self, idx: [usize; 3]) -> &mut Self::Output {
        if self.is_uniform() {
            return &mut self.blocks[0];
        } else {
            return &mut self.blocks[idx[0] * Chunk::SIZE.pow(2) + idx[2] * Chunk::SIZE + idx[1]];
        }
    }
}

impl Index<IVec3> for Chunk {
    type Output = BlockId;

    fn index(&self, idx: IVec3) -> &Self::Output {
        if self.is_uniform() {
            return &self.blocks[0];
        } else {
            let idx = utils::world_position_to_block_index(idx);
            return &self.blocks[idx];
        }
    }
}

impl IndexMut<IVec3> for Chunk {
    fn index_mut(&mut self, idx: IVec3) -> &mut Self::Output {
        if self.is_uniform() {
            return &mut self.blocks[0];
        } else {
            let idx = utils::world_position_to_block_index(idx);
            return &mut self.blocks[idx];
        }
    }
}

#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub enum ChunkFace {
    // Forward is +z direction
    Top,
    Bottom,
    Right,
    Left,
    Front,
    Back,
    None,
}

impl ChunkFace {
    pub fn opposite(&self) -> Self {
        match self {
            &ChunkFace::Front => ChunkFace::Back,
            &ChunkFace::Back => ChunkFace::Front,
            &ChunkFace::Right => ChunkFace::Left,
            &ChunkFace::Left => ChunkFace::Right,
            &ChunkFace::Top => ChunkFace::Bottom,
            &ChunkFace::Bottom => ChunkFace::Top,
            &ChunkFace::None => panic!("Can't get opposite of ChunkFace::None"),
        }
    }

    // TODO: This looks bad. There should be a ChunkPosition struct, it has the method shift(face:
    // ChunkFace)
    /// Moves the position a chunk's length in the direction of the face.
    pub fn shift_position(&self, mut position: IVec3) -> IVec3 {
        match self {
            ChunkFace::Front => position.z += Chunk::SIZE as i32,
            ChunkFace::Back => position.z -= Chunk::SIZE as i32,
            ChunkFace::Right => position.x += Chunk::SIZE as i32,
            ChunkFace::Left => position.x -= Chunk::SIZE as i32,
            ChunkFace::Top => position.y += Chunk::SIZE as i32,
            ChunkFace::Bottom => position.y -= Chunk::SIZE as i32,
            ChunkFace::None => {}
        }
        return position;
    }
}
