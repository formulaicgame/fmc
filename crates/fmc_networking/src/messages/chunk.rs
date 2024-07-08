use std::collections::HashMap;

use crate::BlockId;
use bevy::prelude::*;
use fmc_networking_derive::{ClientBound, NetworkMessage};
use serde::{Deserialize, Serialize};

/// A chunk of blocks sent to a client
#[derive(NetworkMessage, ClientBound, Serialize, Deserialize, Debug, Clone)]
pub struct Chunk {
    /// The position the chunk takes in the block grid.
    pub position: IVec3,
    // If the chunk is uniform(same block) it's length is 1, else it is CHUNK_SIZE^3.
    // The formula for access is x * CHUNK_SIZE^2 + z * CHUNK_SIZE + y.
    /// The blocks the chunk consists of.
    pub blocks: Vec<BlockId>,
    // Packed u16 containing optional info.
    // bits:
    //     0000 0000 0000 unused
    //     0000
    //       ^^-north/south/east/west
    //      ^---centered
    //     ^----upside down
    pub block_state: HashMap<usize, u16>,
}
