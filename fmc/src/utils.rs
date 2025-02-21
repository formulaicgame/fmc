// use bevy::prelude::IVec3;
//
// use crate::world::chunk::Chunk;
//
// TODO: Make ChunkPosition, replace all occurences of IVec3
// pub fn world_position_to_chunk_position(mut position: IVec3) -> IVec3 {
//     // Removing bits_of(Chunk::SIZE) - 1 is rounding down to nearest CHUNK_SIZE divisible.
//     position = position & !(Chunk::SIZE - 1) as i32;
//     return position;
// }

// TODO: Make (Block)/(Chunk)Index::from<IVec3>(position)
// pub fn world_position_to_block_index(mut position: IVec3) -> usize {
//     // Getting the last 4 bits will output 0->Chunk::SIZE for both positive and negative numbers
//     // because of two's complement.
//     position = position & (Chunk::SIZE - 1) as i32;
//     return (position.x << 8 | position.z << 4 | position.y) as usize;
// }

// TODO: (Block)/(Chunk)Index::to_ivec3?
// pub fn block_index_to_position(index: usize) -> IVec3 {
//     const MASK: usize = Chunk::SIZE - 1;
//     let position = IVec3 {
//         x: index as i32 >> 8,
//         z: (index >> 4 & MASK) as i32,
//         y: (index & MASK) as i32,
//     };
//
//     return position;
// }

// TODO: Remove entirely
// Converts world space coordinates to index in self.chunks and index of block in chunk
// pub fn world_position_to_chunk_position_and_block_index(position: IVec3) -> (IVec3, usize) {
//     let chunk_pos = world_position_to_chunk_position(position);
//     let block_index = world_position_to_block_index(position);
//     return (chunk_pos, block_index);
// }

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
