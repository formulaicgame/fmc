use std::collections::HashMap;

use bevy::prelude::*;

use crate::{
    rendering::chunk::ExpandedChunk,
    utils,
    world::{
        blocks::{BlockFace, BlockId, Blocks, Friction},
        world_map::chunk::Chunk,
    },
};

pub mod chunk;
mod chunk_manager;

pub use chunk_manager::NewChunkEvent;

pub struct WorldMapPlugin;
impl Plugin for WorldMapPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(chunk_manager::ChunkManagerPlugin)
            .init_resource::<WorldMap>();
    }
}

/// Map of all chunks that have been received from the server.
#[derive(Resource, Default)]
pub struct WorldMap {
    // TODO: Might make sense to make this an array of Chunk::SIZE^3. It would help with lookup
    // speed when the chunks need to be marked visible for rendering. I really dislike having to
    // shift all the chunks when passing chunk borders though, even though it might not be very
    // expensive.
    pub chunks: HashMap<IVec3, Chunk>,
}

impl WorldMap {
    //XXX: Maybe future optimization
    //     The idea was to split the world into columns, so that columns with a lot of air could be
    //     traversed quickly when frustum culling.
    ///// Insert an entity of a chunk with blocks, or use None if the chunk is air.
    //// The position of air chunks are used as the position of the nearest chunk below it.
    //// All air chunks therefore "belong" to another chunk, be that another air chunk or one with
    //// blocks.
    //// This is used as an optimization for the view frustum. When the player is at the surface,
    //// it takes too long to check all the air chunks individually. By collecting them in columns
    //// like this it can skip nearly all the air chunks.
    //// See chunk_loading_and_frustum_culling_system for more.
    //pub fn insert(&mut self, position: IVec3, entity: Option<Entity>) {
    //    let mut bottom_chunk_position = position;
    //    let mut y_offset = IVec3::new(0, CHUNK_SIZE as i32, 0);

    //    let time = std::time::Instant::now();
    //    // Add chunk, if air check below it for position of chunk.
    //    if entity.is_some() {
    //        self.chunks.insert(position, Chunk::new(position, entity));
    //    } else {
    //        if let Some(below) = self.get(&(position - y_offset)) {
    //            if below.entity.is_some() {
    //                bottom_chunk_position = position - y_offset;
    //            } else {
    //                bottom_chunk_position = below.column;
    //            }
    //        };
    //        self.chunks
    //            .insert(position, Chunk::new(bottom_chunk_position, None));
    //    };

    //    while let Some(chunk) = self.get_mut(&(bottom_chunk_position + y_offset)) {
    //        if chunk.entity.is_some() {
    //            break;
    //        } else {
    //            chunk.column = bottom_chunk_position;
    //        }
    //        y_offset.y += CHUNK_SIZE as i32;
    //    }

    //    y_offset.y -= CHUNK_SIZE as i32;

    //    // The bottommost chunk stores the position of the topmost chunk.
    //    self.get_mut(&bottom_chunk_position).unwrap().column = y_offset;
    //    dbg!(time.elapsed());
    //}

    pub fn insert(&mut self, position: IVec3, chunk: Chunk) {
        self.chunks.insert(position, chunk);
    }

    pub fn contains_chunk(&self, position: &IVec3) -> bool {
        return self.chunks.contains_key(&position);
    }

    pub fn get_chunk(&self, position: &IVec3) -> Option<&Chunk> {
        return self.chunks.get(&position);
    }

    pub fn get_chunk_mut(&mut self, position: &IVec3) -> Option<&mut Chunk> {
        return self.chunks.get_mut(position);
    }

    pub fn get_block(&self, position: &IVec3) -> Option<BlockId> {
        let chunk_position = utils::world_position_to_chunk_pos(*position);
        if let Some(chunk) = self.get_chunk(&chunk_position) {
            let block_position = utils::world_position_to_block_index(*position);
            return Some(chunk[block_position]);
        } else {
            return None;
        }
    }

    /// Find which block the transform is looking at, if any.
    pub fn raycast_to_block(
        &self,
        transform: &Transform,
        origin: IVec3,
        distance: f32,
    ) -> Option<(IVec3, BlockId, BlockFace)> {
        let blocks = Blocks::get();
        let forward = *transform.forward();
        let direction = forward.signum();

        // How far along the forward vector you need to go to hit the next block in each direction.
        // This makes more sense if you mentally align it with the block grid.
        //
        // This relies on some peculiar behaviour where normally f32.fract() would retain the
        // sign of the number, Vec3.fract() instead does self - self.floor(). This results in
        // having the correct value for the negative direction, but it has to be flipped for the
        // positive direction, which is the vec3::select.
        let mut distance_next = transform.translation.fract();
        distance_next = Vec3::select(
            direction.cmpeq(Vec3::ONE),
            1.0 - distance_next,
            distance_next,
        );
        distance_next = distance_next / forward.abs();

        // How much you need to advance along the forward vector to traverse one block in each direction.
        let t_block = 1.0 / forward.abs();
        // +/-1 to shift block_pos when it hits the grid
        let step = direction.as_ivec3();

        // The origin of the ray.
        let mut block_pos = transform.translation.floor().as_ivec3() + origin;

        while (distance_next.min_element() * forward).length_squared() < distance.powi(2) {
            if distance_next.x < distance_next.y && distance_next.x < distance_next.z {
                block_pos.x += step.x;
                distance_next.x += t_block.x;

                if let Some(block_id) = self.get_block(&block_pos) {
                    // TODO: Function needs to take a flag for if it should pass through blocks
                    // with drag. Or maybe return both position of first drag block and first
                    // solid. Do this for server too.
                    if let Friction::Drag(_) = blocks.get_config(block_id).friction() {
                        continue;
                    }

                    let block_side = if direction.x == 1.0 {
                        BlockFace::Left
                    } else {
                        BlockFace::Right
                    };

                    return Some((block_pos, block_id, block_side));
                }
            } else if distance_next.z < distance_next.x && distance_next.z < distance_next.y {
                block_pos.z += step.z;
                distance_next.z += t_block.z;

                if let Some(block_id) = self.get_block(&block_pos) {
                    if let Friction::Drag(_) = blocks.get_config(block_id).friction() {
                        continue;
                    }

                    let block_side = if direction.z == 1.0 {
                        BlockFace::Back
                    } else {
                        BlockFace::Front
                    };
                    return Some((block_pos, block_id, block_side));
                }
            } else {
                block_pos.y += step.y;
                distance_next.y += t_block.y;

                if let Some(block_id) = self.get_block(&block_pos) {
                    if let Friction::Drag(_) = blocks.get_config(block_id).friction() {
                        continue;
                    }

                    let block_face = if direction.y == 1.0 {
                        BlockFace::Bottom
                    } else {
                        BlockFace::Top
                    };

                    return Some((block_pos, block_id, block_face));
                }
            }
        }
        return None;
    }

    // Given a chunk position, returns the blocks in that chunk as well as the blocks one past the
    // edge on all sides.
    pub fn get_expanded_chunk(&self, position: IVec3) -> ExpandedChunk {
        let center = self.get_chunk(&position).unwrap().clone();

        let top_position = position + IVec3::new(0, Chunk::SIZE as i32, 0);
        let top = self.get_chunk(&top_position).cloned();

        let bottom_position = position - IVec3::new(0, Chunk::SIZE as i32, 0);
        let bottom = self.get_chunk(&bottom_position).cloned();

        let right_position = position + IVec3::new(Chunk::SIZE as i32, 0, 0);
        let right = self.get_chunk(&right_position).cloned();

        let left_position = position - IVec3::new(Chunk::SIZE as i32, 0, 0);
        let left = self.get_chunk(&left_position).cloned();

        let front_position = position + IVec3::new(0, 0, Chunk::SIZE as i32);
        let front = self.get_chunk(&front_position).cloned();

        let back_position = position - IVec3::new(0, 0, Chunk::SIZE as i32);
        let back = self.get_chunk(&back_position).cloned();

        return ExpandedChunk {
            center,
            top,
            bottom,
            right,
            left,
            front,
            back,
        };
    }
}
