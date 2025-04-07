use std::{collections::HashMap, sync::Arc};

use crate::{
    bevy::math::DVec3,
    blocks::{BlockId, BlockPosition, BlockState},
    prelude::*,
    world::{chunk::Chunk, terrain_generation::TerrainGenerator},
};

use super::chunk::ChunkPosition;

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
    distance_next: DVec3,
    distance_increment: DVec3,
    current_block_position: BlockPosition,
    step: IVec3,
}

impl<'a> WorldMapRayCast<'a> {
    fn new(world_map: &'a WorldMap, ray_transform: &Transform, max_distance: f64) -> Self {
        let forward = ray_transform.forward();
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
        let mut distance_next = ray_transform.translation.fract_gl();
        distance_next = DVec3::select(
            direction.cmpeq(DVec3::ONE),
            1.0 - distance_next,
            distance_next,
        );
        distance_next = distance_next / forward.abs();

        // How far along the forward vector you need to go to traverse one block in each direction.
        let distance_increment = 1.0 / forward.abs();
        // +/-1 to shift block_pos when it hits the grid
        let step = direction.as_ivec3();

        let current_block_position = BlockPosition::from(ray_transform.translation);

        Self {
            world_map,
            max_distance,
            forward,
            distance_next,
            distance_increment,
            current_block_position,
            step,
        }
    }

    pub fn position(&self) -> BlockPosition {
        self.current_block_position
    }

    pub fn next_block(&mut self) -> Option<BlockId> {
        let next = self.distance_next.min_element();

        if (self.distance_next.min_element() * self.forward).length_squared()
            > self.max_distance.powi(2)
        {
            return None;
        }

        if self.distance_next.x == next {
            self.current_block_position.x += self.step.x;
            self.distance_next.x += self.distance_increment.x;
        } else if self.distance_next.z == next {
            self.current_block_position.z += self.step.z;
            self.distance_next.z += self.distance_increment.z;
        } else {
            self.current_block_position.y += self.step.y;
            self.distance_next.y += self.distance_increment.y;
        }

        // TODO: Probably wise to cache the chunk
        return self.world_map.get_block(self.current_block_position);
    }
}
