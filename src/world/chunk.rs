use std::collections::HashMap;
use std::ops::{Index, IndexMut};

use bevy::prelude::*;

use crate::blocks::{BlockId, BlockState};

use super::BlockPosition;

#[derive(Component)]
pub struct ChunkMarker;

#[derive(Clone)]
pub struct Chunk {
    // The chunk only has an entity if it is rendered. Most chunks are "air" so do not need to take
    // up resources.
    entity: Option<Entity>,
    // NOTE: Notice that the coordinates align with the rendering world, the z axis extends
    // out of the screen. 0,0,0 is the bottom left FAR corner. Not bottom left NEAR.
    // A Chunk::SIZE^3 array containing all the blocks in the chunk.
    // Indexed by x * Chunk::SIZE^2 + z * CHUNK_SIZE + y
    blocks: Vec<BlockId>,
    // Optional block state
    block_state: HashMap<usize, BlockState>,
}

impl Chunk {
    pub const SIZE: usize = 16;

    /// Build a normal chunk
    pub fn new(
        entity: Option<Entity>,
        blocks: Vec<BlockId>,
        block_state: HashMap<usize, BlockState>,
    ) -> Self {
        return Self {
            entity,
            blocks,
            block_state,
        };
    }

    pub fn convert_uniform_to_full(&mut self, entity: Entity) {
        if !self.is_uniform() {
            // Guards against hanging the chunk entity too
            panic!("Tried to convert a non uniform chunk");
        }
        let block = self.blocks[0];
        self.blocks = vec![block; Chunk::SIZE.pow(3)];
        self.entity = Some(entity);
    }

    pub fn entity(&self) -> Option<Entity> {
        self.entity
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

    pub fn iter_blocks(&self) -> std::iter::Enumerate<std::slice::Iter<'_, BlockId>> {
        self.blocks.iter().enumerate()
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

impl Index<BlockPosition> for Chunk {
    type Output = BlockId;

    fn index(&self, idx: BlockPosition) -> &Self::Output {
        if self.is_uniform() {
            return &self.blocks[0];
        } else {
            return &self.blocks[idx.as_chunk_index()];
        }
    }
}

impl IndexMut<BlockPosition> for Chunk {
    fn index_mut(&mut self, idx: BlockPosition) -> &mut Self::Output {
        if self.is_uniform() {
            return &mut self.blocks[0];
        } else {
            return &mut self.blocks[idx.as_chunk_index()];
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
    }}
