use std::collections::{HashMap, HashSet};

use bevy::prelude::*;

use crate::{
    blocks::{BlockId, BlockState},
    utils,
};

use super::chunk::Chunk;

pub mod blueprints;

pub trait TerrainGenerator: Send + Sync {
    fn generate_chunk(&self, position: IVec3) -> Chunk;
}

pub struct TerrainFeature {
    /// The blocks the feature consists of segmented into the chunks they are a part of.
    pub blocks: HashMap<IVec3, Vec<(usize, BlockId, Option<u16>)>>,
    // TODO: Replacement rules should be more granular. Blueprints may consist of many
    // sub-blueprints that each have their own replacement rules that should be followed only for
    // that blueprint.
    // TODO: This is really inefficient. Most features will match against a single block like air
    // or stone, it doesn't make sense to do a lookup, might even be best to do linear search.
    // Enum {
    //     Any, Can replace anything
    //     Single(BlockId), Can replace a single block, fast comparison
    //     Afew([Option<BlockId>; 5]), If there's 2-5 replace rules this is probably faster to search
    //     Many(Hashset<BlockId>), If there are more, benchmark length when faster to do lookup
    //     than search the above.
    //     Magic(...), You probably want some way to do "if replacing this block, use that block",
    //     like ores for different types of stone.
    // }
    // https://gist.github.com/daboross/976978d8200caf86e02acb6805961195 says really long at bottom
    pub can_replace: HashSet<BlockId>,
}

impl TerrainFeature {
    fn insert_block(&mut self, position: IVec3, block_id: BlockId) {
        let (chunk_position, block_index) =
            utils::world_position_to_chunk_position_and_block_index(position);
        self.blocks
            .entry(chunk_position)
            .or_insert(Vec::new())
            .push((block_index, block_id, None));
    }

    // TODO: Is it possible to make it so that features can fail? There are many things that just
    // don't look very good when partially placed. Failure means it would have to revert to the
    // previous state, which is not an easy task. The features are applied to chunks as the chunks
    // are generated, and changing the block to then set it back again does not seem plausible.
    // There would have to be some notification system I suppose that triggers a feature
    // application when all the chunks it will apply to have been generated and are in memory. Then
    // it can check all placements as the first thing it does, then apply if it succeeds. Sounds
    // expensive though.
    pub fn apply(&self, chunk: &mut Chunk, chunk_position: IVec3) {
        if let Some(feature_blocks) = self.blocks.get(&chunk_position) {
            for (block_index, block_id, block_state) in feature_blocks {
                if !chunk.changed_blocks.contains(block_index)
                    && self.can_replace.contains(&chunk[*block_index])
                {
                    chunk[*block_index] = *block_id;
                    chunk.set_block_state(*block_index, block_state.map(BlockState));
                }
            }
        }
    }

    // Applies the feature and returns the blocks that were changed. Used for updating chunks that
    // have already been sent to the clients.
    pub fn apply_return_changed(
        &self,
        chunk: &mut Chunk,
        chunk_position: IVec3,
    ) -> Option<Vec<(usize, BlockId, Option<u16>)>> {
        if let Some(mut feature_blocks) = self.blocks.get(&chunk_position).cloned() {
            feature_blocks.retain(|(block_index, block_id, block_state)| {
                if !chunk.changed_blocks.contains(block_index)
                    && self.can_replace.contains(&chunk[*block_index])
                {
                    chunk[*block_index] = *block_id;
                    chunk.set_block_state(*block_index, block_state.map(BlockState));
                    true
                } else {
                    false
                }
            });

            if feature_blocks.len() > 0 {
                return Some(feature_blocks);
            }
        }

        return None;
    }
}
