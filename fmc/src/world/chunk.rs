use bevy::math::DVec3;
use bevy::prelude::*;
use std::collections::{HashMap, HashSet};
use std::ops::{Add, Index, IndexMut, Sub};
use std::sync::Arc;

use crate::blocks::{BlockData, BlockPosition};
use crate::{
    blocks::{BlockId, BlockState, Blocks},
    database::Database,
    utils,
};

use super::terrain_generation::{TerrainFeature, TerrainGenerator};

// XXX: block_state is used by the database to mark uniform chunks by setting it to
// u16::MAX(an otherwise invalid state).
#[derive(Default)]
pub struct Chunk {
    // All blocks that have been changed in the chunk. These are kept in memory at runtime to allow
    // applying neighbour chunk's terrain features without overwriting.
    pub changed_blocks: HashSet<usize>,
    // This stores the edge features of the chunk + the features that contain bounding boxes that
    // need to be checked against when applying edge features.
    pub terrain_features: Vec<TerrainFeature>,
    // Blocks are stored as an array. To access a block at the coordinate x,y,z
    // the formula x * Chunk::SIZE^2 + z * Chunk::SIZE + y is used.
    pub blocks: Vec<BlockId>,
    // Block state containing optional information, see `BlockState` for bit layout. Saved as u16
    // because it is sent to the players.
    pub block_state: HashMap<usize, u16>,
    // Entities that belong to the blocks of the chunk. An entity is spawned for any block that
    // has a spawning function in its block config or if the block is represented by a model.
    pub block_entities: HashMap<usize, Entity>,
    // TODO: I don't like storing temporary stuff in a permanent structure
    //
    // Temporary storage for the block entities' blockdata, until it is transfered to entities.
    pub block_data: HashMap<usize, BlockData>,
    // Which chunk faces within the chunk are visible from one another.
    visible_faces: HashSet<(ChunkFace, ChunkFace)>,
}

impl Chunk {
    pub const SIZE: usize = 16;

    pub async fn load(
        chunk_position: ChunkPosition,
        terrain_generator: Arc<dyn TerrainGenerator>,
        database: Database,
    ) -> (ChunkPosition, Chunk) {
        let mut chunk = terrain_generator.generate_chunk(chunk_position);

        let changed_blocks = database.load_chunk_blocks(&chunk_position);
        for (index, (block_id, maybe_block_state, maybe_block_data)) in changed_blocks {
            chunk.changed_blocks.insert(index);
            chunk[index] = block_id;
            if let Some(block_state) = maybe_block_state {
                chunk.block_state.insert(index, block_state.as_u16());
            }
            if let Some(block_data) = maybe_block_data {
                chunk.block_data.insert(index, block_data);
            }
        }

        chunk.check_visible_faces();

        return (chunk_position, chunk);
    }

    pub fn make_uniform(&mut self, block_id: BlockId) {
        self.blocks = vec![block_id; 1];
    }

    pub fn is_uniform(&self) -> bool {
        return self.blocks.len() == 1;
    }

    fn convert_uniform_to_regular(&mut self) {
        let block_id = self.blocks[0];
        self.blocks = vec![block_id; Self::SIZE.pow(3)];
    }

    pub fn set_block(&mut self, index: usize, block_id: BlockId) -> BlockId {
        std::mem::replace(&mut self.blocks[index], block_id)
    }

    pub fn get_block_state(&self, index: &usize) -> Option<BlockState> {
        return self.block_state.get(index).copied().map(BlockState);
    }

    pub fn set_block_state(
        &mut self,
        block_index: usize,
        block_state: Option<BlockState>,
    ) -> Option<BlockState> {
        if let Some(block_state) = block_state {
            self.block_state
                .insert(block_index, block_state.as_u16())
                .map(BlockState)
        } else {
            self.block_state.remove(&block_index).map(BlockState)
        }
    }

    pub fn is_neighbour_visible(&self, from: ChunkFace, to: ChunkFace) -> bool {
        return self.visible_faces.contains(&(from, to));
    }

    // TODO: This is expensive and needs to be recomputed every time a block changes. I don't think
    // it is tenable with many players. Best way I can think of is to color the cells visited, then
    // when changing a block, check if the adjacent blocks have different colors and merge them.
    // Breaking the color limit is no big deal, an inaccurate result will only result in an
    // extra chunk load or two.
    pub(super) fn check_visible_faces(&mut self) {
        let blocks = Blocks::get();

        self.visible_faces.clear();

        let mut visited = [false; Self::SIZE.pow(3)];

        const FACES: [ChunkFace; 6] = [
            ChunkFace::Top,
            ChunkFace::Bottom,
            ChunkFace::Right,
            ChunkFace::Left,
            ChunkFace::Front,
            ChunkFace::Back,
        ];

        if self.is_uniform() {
            if blocks.get_config(&self[0]).is_transparent() {
                for face in FACES {
                    for other_face in FACES {
                        self.visible_faces
                            .insert((face.clone(), other_face.clone()));
                        self.visible_faces
                            .insert((other_face.clone(), face.clone()));
                    }
                }
            }
            return;
        }

        let mut stack = Vec::new();

        for i in 0..Self::SIZE as i32 {
            for j in 0..Self::SIZE as i32 {
                for k in (0..Self::SIZE as i32).step_by(Self::SIZE - 1) {
                    let front_back = BlockPosition::new(i, j, k);
                    let left_right = BlockPosition::new(k, i, j);
                    let top_bottom = BlockPosition::new(i, k, j);
                    for source_position in [front_back, left_right, top_bottom] {
                        stack.push(source_position);

                        // TODO: This is too heavy, maybe ChunkFace could be a bitmask and
                        // you can just | them.
                        let mut seen = HashSet::new();

                        while let Some(position) = stack.pop() {
                            match ChunkFace::from_position(&position) {
                                ChunkFace::None => (),
                                face => {
                                    seen.insert(face);
                                    // This position is outside the chunk, skip to next position
                                    continue;
                                }
                            }

                            let index = position.as_chunk_index();
                            if !visited[index] && blocks.get_config(&self[index]).is_transparent() {
                                visited[index] = true;
                                for offset in [
                                    IVec3::X,
                                    IVec3::NEG_X,
                                    IVec3::Y,
                                    IVec3::NEG_Y,
                                    IVec3::Z,
                                    IVec3::NEG_Z,
                                ] {
                                    stack.push(position + offset);
                                }
                            }
                        }

                        for face in seen.iter() {
                            for other_face in seen.iter() {
                                self.visible_faces
                                    .insert((face.clone(), other_face.clone()));
                                self.visible_faces
                                    .insert((other_face.clone(), face.clone()));
                            }
                        }
                    }
                }
            }
        }
    }
}

// Index a chunk by `chunk[[x,y,z]]`
impl Index<[usize; 3]> for Chunk {
    type Output = BlockId;

    fn index(&self, idx: [usize; 3]) -> &Self::Output {
        if self.is_uniform() {
            return &self.blocks[0];
        } else {
            return &self.blocks[idx[0] * Self::SIZE.pow(2) + idx[2] * Self::SIZE + idx[1]];
        }
    }
}

impl IndexMut<[usize; 3]> for Chunk {
    fn index_mut(&mut self, idx: [usize; 3]) -> &mut Self::Output {
        if self.is_uniform() {
            self.convert_uniform_to_regular();
        }
        return &mut self.blocks[idx[0] * Self::SIZE.pow(2) + idx[2] * Self::SIZE + idx[1]];
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
            self.convert_uniform_to_regular();
        }
        return &mut self.blocks[idx];
    }
}

#[derive(Deref, DerefMut, Hash, Clone, Copy, PartialEq, Eq, Debug)]
pub struct ChunkPosition(pub IVec3);

impl ChunkPosition {
    pub fn new(x: i32, y: i32, z: i32) -> Self {
        let block_position = BlockPosition::new(x, y, z);
        Self::from(block_position)
    }

    pub fn neighbourhood(&self) -> [Self; 27] {
        let mut neighbourhood = [ChunkPosition::from(IVec3::ZERO); 27];
        let mut index = 0;
        for x_offset in &[IVec3::X, IVec3::NEG_X, IVec3::ZERO] {
            for y_offset in &[IVec3::Y, IVec3::NEG_Y, IVec3::ZERO] {
                for z_offset in &[IVec3::Z, IVec3::NEG_Z, IVec3::ZERO] {
                    neighbourhood[index] = ChunkPosition::from(
                        self.0
                            + x_offset * Chunk::SIZE as i32
                            + y_offset * Chunk::SIZE as i32
                            + z_offset * Chunk::SIZE as i32,
                    );

                    index += 1;
                }
            }
        }

        neighbourhood
    }
}

impl From<BlockPosition> for ChunkPosition {
    fn from(mut value: BlockPosition) -> Self {
        // Removing bits_of(Chunk::SIZE) - 1 is rounding down to nearest CHUNK_SIZE divisible.
        *value = value.0 & !(Chunk::SIZE - 1) as i32;
        return Self(*value);
    }
}

impl From<&BlockPosition> for ChunkPosition {
    fn from(value: &BlockPosition) -> Self {
        // Removing bits_of(Chunk::SIZE) - 1 is rounding down to nearest CHUNK_SIZE divisible.
        return Self(value.0 & !(Chunk::SIZE - 1) as i32);
    }
}

impl From<DVec3> for ChunkPosition {
    fn from(value: DVec3) -> Self {
        let block_position = BlockPosition::from(value);
        Self::from(block_position)
    }
}

impl From<IVec3> for ChunkPosition {
    fn from(value: IVec3) -> Self {
        // Removing bits_of(Chunk::SIZE) - 1 is rounding down to nearest CHUNK_SIZE divisible.
        Self(value & !(Chunk::SIZE - 1) as i32)
    }
}

impl Add<ChunkPosition> for ChunkPosition {
    type Output = ChunkPosition;

    fn add(self, rhs: ChunkPosition) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl Sub<ChunkPosition> for ChunkPosition {
    type Output = ChunkPosition;

    fn sub(self, rhs: ChunkPosition) -> Self::Output {
        Self(self.0 - rhs.0)
    }
}

#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub enum ChunkFace {
    Top,
    Bottom,
    Right,
    Left,
    // +z direction
    Front,
    Back,
    None,
}

impl ChunkFace {
    pub fn opposite(&self) -> Self {
        match self {
            ChunkFace::Front => ChunkFace::Back,
            ChunkFace::Back => ChunkFace::Front,
            ChunkFace::Right => ChunkFace::Left,
            ChunkFace::Left => ChunkFace::Right,
            ChunkFace::Top => ChunkFace::Bottom,
            ChunkFace::Bottom => ChunkFace::Top,
            ChunkFace::None => panic!("Can't get opposite of ChunkFace::None"),
        }
    }

    /// Moves the position a chunk's length in the direction of the face.
    pub fn shift_position(&self, mut chunk_position: ChunkPosition) -> ChunkPosition {
        match self {
            ChunkFace::Front => chunk_position.z += Chunk::SIZE as i32,
            ChunkFace::Back => chunk_position.z -= Chunk::SIZE as i32,
            ChunkFace::Right => chunk_position.x += Chunk::SIZE as i32,
            ChunkFace::Left => chunk_position.x -= Chunk::SIZE as i32,
            ChunkFace::Top => chunk_position.y += Chunk::SIZE as i32,
            ChunkFace::Bottom => chunk_position.y -= Chunk::SIZE as i32,
            ChunkFace::None => {}
        }
        return chunk_position;
    }

    /// Returns the chunk face the vector placed in the middle of the chunk points at.
    pub fn convert_vector(vec: &Vec3) -> Self {
        let abs = vec.abs();
        if abs.x > abs.y && abs.x > abs.z {
            if vec.x < 0.0 {
                return ChunkFace::Left;
            } else {
                return ChunkFace::Right;
            }
        } else if abs.y > abs.x && abs.y > abs.z {
            if vec.y < 0.0 {
                return ChunkFace::Bottom;
            } else {
                return ChunkFace::Top;
            }
        } else {
            if vec.z < 0.0 {
                return ChunkFace::Back;
            } else {
                return ChunkFace::Front;
            }
        }
    }

    /// Given a relative block position that is immediately adjacent to one of the chunk's faces, return the face.
    pub fn from_position(position: &IVec3) -> Self {
        if position.z > (Chunk::SIZE - 1) as i32 {
            return ChunkFace::Front;
        } else if position.z < 0 {
            return ChunkFace::Back;
        } else if position.x > (Chunk::SIZE - 1) as i32 {
            return ChunkFace::Right;
        } else if position.x < 0 {
            return ChunkFace::Left;
        } else if position.y > (Chunk::SIZE - 1) as i32 {
            return ChunkFace::Top;
        } else if position.y < 0 {
            return ChunkFace::Bottom;
        } else {
            return ChunkFace::None;
        }
    }
}
