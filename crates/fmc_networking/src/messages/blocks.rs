use bevy::prelude::*;

use serde::{Deserialize, Serialize};

use fmc_networking_derive::{ClientBound, NetworkMessage};

use crate::BlockId;

/// Change individual blocks.
#[derive(NetworkMessage, ClientBound, Serialize, Deserialize, Debug, Clone)]
pub struct BlockUpdates {
    /// The position of the chunk that is to be changed.
    pub chunk_position: IVec3,
    /// A list of blocks to update
    pub blocks: Vec<(usize, BlockId, Option<u16>)>,
}
