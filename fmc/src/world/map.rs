use bevy::{math::DVec3, prelude::*};
use std::{collections::HashMap, sync::Arc};

use crate::{
    bevy_extensions::f64_transform::Transform,
    blocks::{BlockFace, BlockId, BlockRotation, BlockState, Blocks},
    utils,
    world::{chunk::Chunk, terrain_generation::TerrainGenerator},
};

#[derive(Resource)]
pub struct WorldMap {
    chunks: HashMap<IVec3, Chunk>,
    pub terrain_generator: Arc<dyn TerrainGenerator>,
}

impl WorldMap {
    pub fn new(terrain_generator: impl TerrainGenerator + 'static) -> Self {
        Self {
            chunks: HashMap::new(),
            terrain_generator: Arc::new(terrain_generator),
        }
    }

    pub fn contains_chunk(&self, chunk_position: &IVec3) -> bool {
        return self.chunks.contains_key(chunk_position);
    }

    pub fn get_chunk(&self, position: &IVec3) -> Option<&Chunk> {
        return self.chunks.get(&position);
    }

    pub fn get_chunk_mut(&mut self, chunk_position: &IVec3) -> Option<&mut Chunk> {
        return self.chunks.get_mut(&chunk_position);
    }

    pub fn insert(&mut self, chunk_position: IVec3, value: Chunk) {
        self.chunks.insert(chunk_position, value);
    }

    pub fn remove_chunk(&mut self, chunk_position: &IVec3) {
        self.chunks.remove(chunk_position);
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
        max_distance: f64,
        // (position, block id, block face, distance to hit)
    ) -> Option<(IVec3, BlockId, BlockFace, f64)> {
        let blocks = Blocks::get();
        let forward = transform.forward();
        let direction = forward.signum();

        // How far along the forward vector you need to go to hit the next block in each direction.
        // This makes more sense if you mentally align it with the block grid.
        //
        // TODO: This fract stuff can probably be simplified now, they made fract() correct.
        // Using fract_gl instead, as that is now the old functionality.
        //
        // This relies on some peculiar behaviour where normally f32.fract() would retain the
        // sign of the number, Vec3.fract() instead does self - self.floor(). This results in
        // having the correct value for the negative direction, but it has to be flipped for the
        // positive direction, which is the vec3::select.
        let mut distance_next = transform.translation.fract_gl();
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

        let mut block_face;
        let mut ray_length;
        let mut next = distance_next.min_element();
        while (next * forward).length_squared() < max_distance.powi(2) {
            if distance_next.x == next {
                block_pos.x += step.x;
                distance_next.x += t_block.x;

                block_face = if direction.x == 1.0 {
                    BlockFace::Left
                } else {
                    BlockFace::Right
                };

                ray_length = distance_next.x - t_block.x;
            } else if distance_next.z == next {
                block_pos.z += step.z;
                distance_next.z += t_block.z;

                block_face = if direction.z == 1.0 {
                    BlockFace::Back
                } else {
                    BlockFace::Front
                };

                ray_length = distance_next.z - t_block.z;
            } else {
                block_pos.y += step.y;
                distance_next.y += t_block.y;

                block_face = if direction.y == 1.0 {
                    BlockFace::Bottom
                } else {
                    BlockFace::Top
                };

                ray_length = distance_next.y - t_block.y;
            }

            next = distance_next.min_element();

            if let Some(block_id) = self.get_block(block_pos) {
                let block_config = blocks.get_config(&block_id);
                if block_config.hardness.is_none() || block_config.model.is_some() {
                    continue;
                }

                let rotation = self
                    .get_block_state(block_pos)
                    .map(BlockState::rotation)
                    .flatten()
                    .map(BlockRotation::as_quat)
                    .unwrap_or_default();

                let block_transform = Transform {
                    translation: block_pos.as_dvec3(),
                    rotation,
                    ..default()
                };

                if let Some(hitbox) = &block_config.hitbox {
                    if let Some(length) =
                        hitbox.ray_intersection(transform.translation, forward, block_transform)
                    {
                        ray_length += length;
                    } else {
                        continue;
                    }
                }

                return Some((block_pos, block_id, block_face, ray_length));
            }
        }
        return None;
    }
}
