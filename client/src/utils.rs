use bevy::prelude::*;

use crate::world::world_map::chunk::Chunk;

// TODO: Move all three to world_map I believe
// Translates a block position into the chunk position of the chunk the block is in.
pub fn world_position_to_chunk_pos(mut position: IVec3) -> IVec3 {
    // Removing bits of Chunk::SIZE - 1 (0b1111) is rounding down to nearest Chunk::SIZE divisible.
    position = position & !(Chunk::SIZE - 1) as i32;
    return position;
}

// Translates a block position into the index of the block in its chunk.
pub fn world_position_to_block_index(mut position: IVec3) -> usize {
    // Getting the last 4 bits will output 0->Chunk::SIZE for both positive and negative numbers
    // because of two's complement.
    position = position & (Chunk::SIZE - 1) as i32;
    return (position.x << 8 | position.z << 4 | position.y) as usize;
}

// Translates a block position into the position of the chunk that contains it, and the index of
// the block position inside the chunk.
pub fn world_position_to_chunk_position_and_block_index(position: IVec3) -> (IVec3, usize) {
    let chunk_coord = world_position_to_chunk_pos(position);
    let block_coord = world_position_to_block_index(position);

    return (chunk_coord, block_coord);
}

pub fn block_index_to_position(index: usize) -> IVec3 {
    const MASK: usize = Chunk::SIZE - 1;
    let position = IVec3 {
        x: index as i32 >> 8,
        z: (index >> 4 & MASK) as i32,
        y: (index & MASK) as i32,
    };

    return position;
}

#[derive(Default, Debug, Clone)]
pub struct Rng {
    seed: u64,
}

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self { seed }
    }

    pub fn next_u32(&mut self) -> u32 {
        let seed = self.seed.wrapping_add(0x2d35_8dcc_aa6c_78a5);
        self.seed = seed;
        let t = u128::from(seed) * u128::from(seed ^ 0x8bb8_4b93_962e_acc9);
        return ((t as u64) ^ (t >> 64) as u64) as u32;
    }

    pub fn next_f32(&mut self) -> f32 {
        let seed = self.seed.wrapping_add(0x2d35_8dcc_aa6c_78a5);
        self.seed = seed;
        let t = u128::from(seed) * u128::from(seed ^ 0x8bb8_4b93_962e_acc9);
        let result = ((t as u64) ^ (t >> 64) as u64) as u32;
        // Only want 23 bits of the result for the mantissa, rest is discarded and replaced
        // with exponent of 127 so the result is in range 1..2 then -1 to move the range down
        // to 0..1
        f32::from_bits((result >> 9) | (127 << 23)) - 1.0
    }
}
