use std::{collections::HashMap, sync::Arc};

use crate::{
    bevy::math::DVec3,
    blocks::{BlockId, BlockPosition, BlockState},
    prelude::*,
    world::{chunk::Chunk, terrain_generation::TerrainGenerator},
};

use super::chunk::ChunkPosition;

/// Holds the world's chunks. You must insert this yourself with the [TerrainGenerator] you want.
#[derive(Resource)]
pub struct WorldMap {
    chunks: HashMap<ChunkPosition, Chunk>,
    pub terrain_generator: Arc<dyn TerrainGenerator>,
}

impl WorldMap {
    pub fn new(terrain_generator: impl TerrainGenerator + 'static) -> Self {
        Self {
            chunks: HashMap::new(),
            terrain_generator: Arc::new(terrain_generator),
        }
    }

    pub fn contains_chunk(&self, chunk_position: &ChunkPosition) -> bool {
        return self.chunks.contains_key(chunk_position);
    }

    pub fn get_chunk(&self, chunk_position: &ChunkPosition) -> Option<&Chunk> {
        return self.chunks.get(chunk_position);
    }

    pub fn get_chunk_mut(&mut self, chunk_position: &ChunkPosition) -> Option<&mut Chunk> {
        return self.chunks.get_mut(chunk_position);
    }

    pub fn insert(&mut self, chunk_position: ChunkPosition, value: Chunk) {
        self.chunks.insert(chunk_position, value);
    }

    pub fn remove_chunk(&mut self, chunk_position: &ChunkPosition) -> Option<Chunk> {
        self.chunks.remove(chunk_position)
    }

    pub fn get_block(&self, block_position: BlockPosition) -> Option<BlockId> {
        let chunk_position = ChunkPosition::from(*block_position);

        if let Some(chunk) = self.get_chunk(&chunk_position) {
            let index = block_position.as_chunk_index();
            Some(chunk[index])
        } else {
            None
        }
    }

    pub fn get_block_state(&self, position: BlockPosition) -> Option<BlockState> {
        let chunk_position = ChunkPosition::from(position);

        if let Some(chunk) = self.get_chunk(&chunk_position) {
            let index = position.as_chunk_index();
            return chunk.get_block_state(&index);
        } else {
            return None;
        }
    }

    /// Iterator over all the blocks the ray goes through.
    pub fn raycast(&self, ray_transform: &Transform, max_distance: f64) -> WorldMapRayCast {
        WorldMapRayCast::new(self, ray_transform, max_distance)
    }
}

pub struct WorldMapRayCast<'a> {
    world_map: &'a WorldMap,
    max_distance: f64,
    forward: DVec3,
    current_distance: f64,
    current_block_position: BlockPosition,
    distance_to_next: DVec3,
    distance_increment: DVec3,
    step: IVec3,
}

impl<'a> WorldMapRayCast<'a> {
    fn new(world_map: &'a WorldMap, ray_transform: &Transform, max_distance: f64) -> Self {
        let forward = ray_transform.forward();
        let direction = forward.signum();

        // How far along the forward vector you need to go to hit the next block in each direction.
        //
        // fract_gl() uses x - x.floor(), which yields the correct value for all values with a
        // negative direction, e.g. fract_gl(-1.32) = 0.68. When the direction is positive it is
        // just inverted.
        let mut distance_to_next = ray_transform.translation.fract_gl();
        distance_to_next = DVec3::select(
            direction.cmpeq(DVec3::ONE),
            1.0 - distance_to_next,
            distance_to_next,
        );
        distance_to_next = distance_to_next / forward.abs();

        // How far along the forward vector you need to go to traverse one block in each direction.
        let distance_increment = 1.0 / forward.abs();
        // +/-1 to shift block_pos when it hits the grid
        let step = direction.as_ivec3();

        let current_block_position = BlockPosition::from(ray_transform.translation);

        Self {
            world_map,
            max_distance,
            forward,
            current_distance: 0.0,
            current_block_position,
            distance_to_next,
            distance_increment,
            step,
        }
    }

    pub fn distance(&self) -> f64 {
        self.current_distance
    }

    pub fn position(&self) -> BlockPosition {
        self.current_block_position
    }

    pub fn next_block(&mut self) -> Option<BlockId> {
        self.current_distance = self.distance_to_next.min_element();

        if self.current_distance > self.max_distance {
            return None;
        }

        if self.distance_to_next.x == self.current_distance {
            self.current_block_position.x += self.step.x;
            self.distance_to_next.x += self.distance_increment.x;
        } else if self.distance_to_next.z == self.current_distance {
            self.current_block_position.z += self.step.z;
            self.distance_to_next.z += self.distance_increment.z;
        } else {
            self.current_block_position.y += self.step.y;
            self.distance_to_next.y += self.distance_increment.y;
        }

        // TODO: Probably wise to cache the chunk
        return self.world_map.get_block(self.current_block_position);
    }
}
