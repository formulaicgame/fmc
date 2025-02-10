use std::{
    collections::{HashMap, HashSet},
    ops::Index,
};

use bevy::prelude::*;

use crate::blocks::{BlockId, BlockPosition, BlockState, Blocks};

use super::{
    chunk::{Chunk, ChunkPosition},
    WorldMap,
};

pub mod blueprints;

pub trait TerrainGenerator: Send + Sync {
    fn generate_chunk(&self, position: ChunkPosition) -> Chunk;
}

#[derive(Default)]
pub struct TerrainFeature {
    /// The blocks the feature consists of partitioned into the chunks they are a part of.
    pub blocks: HashMap<ChunkPosition, Vec<(usize, BlockId, Option<u16>)>>,
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
    // Terrain feautres may supply a set of bounding boxes that will restrict the
    // feature so that it is only placed where all blocks within the bounding boxes are
    // replaceable.
    pub bounding_boxes: Vec<(BlockPosition, BlockPosition)>,
}

impl TerrainFeature {
    fn insert_block(&mut self, position: BlockPosition, block_id: BlockId) {
        let chunk_position = ChunkPosition::from(position);
        let index = position.as_chunk_index();
        self.blocks
            .entry(chunk_position)
            .or_insert(Vec::new())
            .push((index, block_id, None));
    }

    fn add_bounding_box(&mut self, min: BlockPosition, max: BlockPosition) {
        assert!(min.cmple(max.0).all());
        self.bounding_boxes.push((min, max));
    }

    pub fn applies_to_chunk(&self, chunk_position: &ChunkPosition) -> bool {
        return self.blocks.contains_key(chunk_position);
    }

    // Check if the blocks of the feature, and its bounding boxes fit inside a single chunk.
    pub fn fits_in_chunk(&self, chunk_position: ChunkPosition) -> bool {
        if self.blocks.len() != 1 || !self.blocks.contains_key(&chunk_position) {
            return false;
        }

        for (min, max) in self.bounding_boxes.iter() {
            let min_chunk_position = ChunkPosition::from(*min);
            let max_chunk_position = ChunkPosition::from(*max);
            if min_chunk_position != chunk_position || max_chunk_position != chunk_position {
                return false;
            }
        }

        return true;
    }

    fn check_bounds(&self, chunk_position: ChunkPosition, chunk: &Chunk, blocks: &Blocks) -> bool {
        // Check against already placed blocks
        for (min, max) in self.bounding_boxes.iter().cloned() {
            for x in min.x..=max.x {
                for z in min.z..=max.z {
                    for y in min.y..=max.y {
                        let block_position = BlockPosition::new(x, y, z);
                        let bounds_chunk_position = ChunkPosition::from(block_position);
                        let block_index = block_position.as_chunk_index();

                        if bounds_chunk_position != chunk_position {
                            continue;
                        }

                        let block_id = chunk[block_index];
                        let block = blocks.get_config(&block_id);

                        if !block.replaceable || !self.can_replace.contains(&block_id) {
                            return false;
                        }
                    }
                }
            }

            for terrain_feature in chunk.terrain_features.iter() {
                for (other_min, other_max) in terrain_feature.bounding_boxes.iter() {
                    if other_max.cmpge(*min).all() && other_min.cmple(*max).all() {
                        return false;
                    }
                }
            }
        }

        return true;
    }

    /// This should only be used on terrain features that place blocks in multiple chunks.
    pub fn apply_edge_feature(
        &self,
        world_map: &mut WorldMap,
    ) -> Vec<(ChunkPosition, Vec<(usize, BlockId, Option<u16>)>)> {
        let mut placed_blocks = Vec::new();

        // First we check that all the chunks are available. We eat the extra lookup time so that
        // we won't have to fail late in the bounds/collision checks as that is very wasteful.
        for chunk_position in self.blocks.keys() {
            if !world_map.contains_chunk(chunk_position) {
                return placed_blocks;
            }
        }

        let blocks = Blocks::get();
        for chunk_position in self.blocks.keys() {
            let chunk = world_map.get_chunk(chunk_position).unwrap();
            // TODO: If it intersects with another edge feature, both will be ignored. It should
            // instead choose the one that is closer to the origin of the world.
            if !self.check_bounds(*chunk_position, chunk, blocks) {
                return placed_blocks;
            };
        }

        for (chunk_position, blocks) in self.blocks.iter() {
            let mut new_blocks = Vec::new();
            let chunk = world_map.get_chunk_mut(chunk_position).unwrap();

            for (block_index, block_id, block_state) in blocks.iter().cloned() {
                if !chunk.changed_blocks.contains(&block_index)
                    && self.can_replace.contains(&chunk[block_index])
                {
                    chunk[block_index] = block_id;
                    chunk.set_block_state(block_index, block_state.map(BlockState));
                    new_blocks.push((block_index, block_id, block_state));
                }
            }

            if !new_blocks.is_empty() {
                placed_blocks.push((*chunk_position, new_blocks));
            }
        }

        return placed_blocks;
    }

    pub fn apply(self, chunk_position: ChunkPosition, chunk: &mut Chunk) {
        if !self.fits_in_chunk(chunk_position) {
            // The feature is part of many chunks, have to wait until they are loaded.
            chunk.terrain_features.push(self);
            return;
        }

        if !self.check_bounds(chunk_position, chunk, Blocks::get()) {
            // The feature is blocked by the terrain or another feature, can't be placed
            return;
        };

        for blocks in self.blocks.values() {
            for (block_index, block_id, block_state) in blocks.iter().cloned() {
                if !chunk.changed_blocks.contains(&block_index)
                    && self.can_replace.contains(&chunk[block_index])
                {
                    chunk[block_index] = block_id;
                    chunk.set_block_state(block_index, block_state.map(BlockState));
                }
            }
        }

        if self.bounding_boxes.len() != 0 {
            // Terrain features with bounding boxes must be available for other terrain features to
            // test against.
            chunk.terrain_features.push(self);
        }

        return;
    }
}

/// Keeps track of the surface of a chunk.
pub struct Surface {
    // (y_index, surface block) of each block column if there is one
    surface_blocks: Vec<Option<(usize, BlockId)>>,
}

impl Surface {
    // TODO: The topmost block in each block column will never be used as we don't know what's
    // above it, might be ignoreable.
    pub fn new(chunk: &Chunk, air: BlockId) -> Self {
        let mut surface_blocks = vec![None; Chunk::SIZE.pow(2)];
        for (column_index, block_column) in chunk.blocks.chunks(Chunk::SIZE).enumerate() {
            let mut air_encountered = false;
            for (y_index, block_id) in block_column.into_iter().enumerate().rev() {
                if air_encountered && *block_id != air {
                    surface_blocks[column_index] = Some((y_index, *block_id));
                    break;
                }
                if *block_id == air {
                    air_encountered = true;
                }
            }
        }

        Self { surface_blocks }
    }
}

impl Index<usize> for Surface {
    type Output = Option<(usize, BlockId)>;
    fn index(&self, index: usize) -> &Self::Output {
        &self.surface_blocks[index]
    }
}
