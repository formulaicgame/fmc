use bevy::prelude::IVec3;

use crate::world::chunk::Chunk;

pub fn world_position_to_chunk_position(mut position: IVec3) -> IVec3 {
    // Removing bits_of(Chunk::SIZE) - 1 is rounding down to nearest CHUNK_SIZE divisible.
    position = position & !(Chunk::SIZE - 1) as i32;
    return position;
}

pub fn world_position_to_block_index(mut position: IVec3) -> usize {
    // Getting the last 4 bits will output 0->Chunk::SIZE for both positive and negative numbers
    // because of two's complement.
    position = position & (Chunk::SIZE - 1) as i32;
    return (position.x << 8 | position.z << 4 | position.y) as usize;
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

// Converts world space coordinates to index in self.chunks and index of block in chunk
pub fn world_position_to_chunk_position_and_block_index(position: IVec3) -> (IVec3, usize) {
    let chunk_pos = world_position_to_chunk_position(position);
    let block_index = world_position_to_block_index(position);
    return (chunk_pos, block_index);
}
