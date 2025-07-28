use bevy::prelude::*;

use serde::{Deserialize, Serialize};

use fmc_protocol_derive::ClientBound;

use crate::BlockId;

/// Change individual blocks.
#[derive(ClientBound, Event, Serialize, Deserialize, Debug, Clone)]
pub struct BlockUpdates {
    /// The position of the chunk that is to be changed.
    pub chunk_position: IVec3,
    /// A list of blocks to update
    pub blocks: Vec<(usize, BlockId, Option<u16>)>,
}
