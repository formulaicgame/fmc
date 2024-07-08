use bevy::{math::DVec3, prelude::*};
use std::{collections::HashMap, sync::Arc};

use crate::{
    bevy_extensions::f64_transform::Transform,
    blocks::{BlockFace, BlockId, BlockState, Blocks},
    utils,
    world::{chunk::Chunk, terrain_generation::TerrainGenerator},
};

#[derive(Resource)]
pub struct WorldMap {
    chunks: HashMap<IVec3, Chunk>,
    pub terrain_generator: Arc<dyn TerrainGenerator>,
    pub max_render_distance: u32,
}

impl WorldMap {
    pub fn new(
        terrain_generator: impl TerrainGenerator + 'static,
        max_render_distance: u32,
    ) -> Self {
        Self {
            chunks: HashMap::new(),
            terrain_generator: Arc::new(terrain_generator),
            max_render_distance,
        }
    }

    pub fn contains_chunk(&self, position: &IVec3) -> bool {
        return self.chunks.contains_key(position);
    }

    pub fn get_chunk(&self, position: &IVec3) -> Option<&Chunk> {
        return self.chunks.get(&position);
    }

    pub fn get_chunk_mut(&mut self, position: &IVec3) -> Option<&mut Chunk> {
        return self.chunks.get_mut(&position);
    }

    pub fn insert(&mut self, position: IVec3, value: Chunk) {
        self.chunks.insert(position, value);
    }

    pub fn remove_chunk(&mut self, position: &IVec3) {
        self.chunks.remove(position);
    }

    pub fn get_block(&self, position: IVec3) -> Option<BlockId> {
        let (chunk_pos, index) = utils::world_position_to_chunk_position_and_block_index(position);

        if let Some(chunk) = self.get_chunk(&chunk_pos) {
            Some(chunk[index])
        } else {
            None
        }
    }

    pub fn get_block_state(&self, position: IVec3) -> Option<BlockState> {
        let (chunk_pos, index) = utils::world_position_to_chunk_position_and_block_index(position);

        if let Some(chunk) = self.get_chunk(&chunk_pos) {
            return chunk.get_block_state(&index);
        } else {
            return None;
        }
    }

    /// Find which block the transform is looking at, if any.
    pub fn raycast_to_block(
        &self,
        transform: &Transform,
        distance: f64,
    ) -> Option<(IVec3, BlockId, BlockFace, f64)> {
        let blocks = Blocks::get();
        let forward = transform.forward();
        let direction = forward.signum();

        // How far along the forward vector you need to go to hit the next block in each direction.
        // This makes more sense if you mentally align it with the block grid.
        //
        // This relies on some peculiar behaviour where normally f32.fract() would retain the
        // sign of the number, Vec3.fract() instead does self - self.floor(). This results in
        // having the correct value for the negative direction, but it has to be flipped for the
        // positive direction, which is the vec3::select.
        let mut distance_next = transform.translation.fract();
        distance_next = DVec3::select(
            direction.cmpeq(DVec3::ONE),
            1.0 - distance_next,
            distance_next,
        );
        distance_next = distance_next / forward.abs();

        // How far along the forward vector you need to go to traverse one block in each direction.
        let t_block = 1.0 / forward.abs();
        // +/-1 to shift block_pos when it hits the grid
        let step = direction.as_ivec3();

        // The origin block of the ray.
        let mut block_pos = transform.translation.floor().as_ivec3();

        while (distance_next.min_element() * forward).length_squared() < distance.powi(2) {
            if distance_next.x < distance_next.y && distance_next.x < distance_next.z {
                block_pos.x += step.x;
                distance_next.x += t_block.x;

                // TODO: Have to do this for each branch, too noisy.
                if let Some(block_id) = self.get_block(block_pos) {
                    if blocks.get_config(&block_id).hardness.is_none() {
                        continue;
                    }

                    let block_side = if direction.x == 1.0 {
                        BlockFace::Left
                    } else {
                        BlockFace::Right
                    };

                    return Some((block_pos, block_id, block_side, distance_next.x - t_block.x));
                }
            } else if distance_next.z < distance_next.x && distance_next.z < distance_next.y {
                block_pos.z += step.z;
                distance_next.z += t_block.z;

                if let Some(block_id) = self.get_block(block_pos) {
                    if blocks.get_config(&block_id).hardness.is_none() {
                        continue;
                    }

                    let block_side = if direction.z == 1.0 {
                        BlockFace::Back
                    } else {
                        BlockFace::Front
                    };
                    return Some((block_pos, block_id, block_side, distance_next.z - t_block.z));
                }
            } else {
                block_pos.y += step.y;
                distance_next.y += t_block.y;

                if let Some(block_id) = self.get_block(block_pos) {
                    if blocks.get_config(&block_id).hardness.is_none() {
                        continue;
                    }

                    let block_side = if direction.y == 1.0 {
                        BlockFace::Bottom
                    } else {
                        BlockFace::Top
                    };

                    return Some((block_pos, block_id, block_side, distance_next.y - t_block.y));
                }
            }
        }
        return None;
    }
}
