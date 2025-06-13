use std::{
    collections::{BinaryHeap, HashMap, HashSet, VecDeque},
    ops::{Index, IndexMut},
};

use bevy::prelude::*;
use fmc_protocol::messages;

use crate::{
    game_state::GameState,
    utils,
    world::{
        blocks::Blocks,
        world_map::{
            chunk::{Chunk, ChunkFace},
            NewChunkEvent, WorldMap,
        },
        Origin,
    },
};

use super::{
    chunk::{ChunkMeshEvent, ExpandedLightChunk},
    RenderSet,
};

pub struct LightingPlugin;
impl Plugin for LightingPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(LightMap::default())
            .add_event::<TestFinishedLightingEvent>()
            .insert_resource(Queues::default())
            .add_systems(
                Update,
                (
                    handle_block_updates.before(propagate_light),
                    propagate_light.after(handle_new_chunks),
                    send_chunk_mesh_events.after(propagate_light),
                    handle_new_chunks,
                    light_chunk_unloading.run_if(resource_changed::<Origin>),
                )
                    .in_set(RenderSet::Light)
                    .run_if(in_state(GameState::Playing)),
            )
            .add_systems(OnEnter(GameState::Launcher), cleanup);
    }
}

fn cleanup(mut light_map: ResMut<LightMap>) {
    light_map.chunks.clear();
}

#[derive(Resource, Default)]
pub struct LightMap {
    chunks: HashMap<IVec3, LightChunk>,
}

impl LightMap {
    pub fn get_light(&self, block_position: IVec3) -> Option<Light> {
        let (chunk_pos, block_index) =
            utils::world_position_to_chunk_position_and_block_index(block_position);
        if let Some(light_chunk) = self.chunks.get(&chunk_pos) {
            Some(light_chunk[block_index])
        } else {
            None
        }
    }

    #[track_caller]
    fn propagate_to_adjacent(
        &mut self,
        chunk_position: IVec3,
        light_update_queues: &mut Queues,
        chunk: &Chunk,
        blocks: &Blocks,
    ) {
        for chunk_face in [
            ChunkFace::Bottom,
            ChunkFace::Top,
            ChunkFace::Right,
            ChunkFace::Left,
            ChunkFace::Front,
            ChunkFace::Back,
        ] {
            let light_chunk = self.chunks.get(&chunk_position).unwrap();

            let adjacent_chunk_position = chunk_face.shift_position(chunk_position);

            let Some(adjacent_light_chunk) = self.chunks.get(&adjacent_chunk_position) else {
                continue;
            };

            if light_chunk.is_uniform_sunlight() && adjacent_light_chunk.is_uniform_sunlight() {
                continue;
            }

            let adjacent_light_queue = light_update_queues
                .entry(adjacent_chunk_position)
                .or_insert(LightUpdateQueue::new());

            if chunk_face == ChunkFace::Bottom {
                adjacent_light_queue.sunlit = true;
            } else {
                adjacent_light_queue.add_timer();
            }

            for i in 0..Chunk::SIZE {
                for j in 0..Chunk::SIZE {
                    let (index, adjacent_index) = match chunk_face {
                        // TODO: All this bit-shifting is confusing, use one of the utility
                        // functions for position perhaps. Or just push ivecs for updates idk.
                        ChunkFace::Top => (i << 8 | j << 4 | (Chunk::SIZE - 1), i << 8 | j << 4),
                        ChunkFace::Bottom => (i << 8 | j << 4, i << 8 | j << 4 | (Chunk::SIZE - 1)),
                        ChunkFace::Right => ((Chunk::SIZE - 1) << 8 | i << 4 | j, i << 4 | j),
                        ChunkFace::Left => (i << 4 | j, (Chunk::SIZE - 1) << 8 | i << 4 | j),
                        ChunkFace::Front => (i << 8 | (Chunk::SIZE - 1) << 4 | j, i << 8 | j),
                        ChunkFace::Back => (i << 8 | j, i << 8 | (Chunk::SIZE - 1) << 4 | j),
                        ChunkFace::None => unreachable!(),
                    };

                    let mut light = light_chunk[index];
                    let block_config = blocks.get_config(chunk[index]);

                    if !light
                        .decrement(block_config.light_attenuation())
                        .can_propagate()
                    {
                        continue;
                    }

                    light = light.decrement(
                        (chunk_face != ChunkFace::Bottom || light.sunlight() != 15) as u8,
                    );

                    if light.sunlight() == 15 {
                        adjacent_light_queue.propagation.push_back(LightUpdate {
                            index: adjacent_index,
                            light,
                        });
                    } else {
                        adjacent_light_queue.propagation.push_front(LightUpdate {
                            index: adjacent_index,
                            light,
                        });
                    }
                }
            }
        }
    }

    pub fn get_expanded_chunk(&self, position: IVec3) -> ExpandedLightChunk {
        let center = self.chunks.get(&position).unwrap().clone();

        let top_position = position + IVec3::new(0, Chunk::SIZE as i32, 0);
        let top_chunk = self.chunks.get(&top_position);

        let bottom_position = position - IVec3::new(0, Chunk::SIZE as i32, 0);
        let bottom_chunk = self.chunks.get(&bottom_position);

        let right_position = position + IVec3::new(Chunk::SIZE as i32, 0, 0);
        let right_chunk = self.chunks.get(&right_position);

        let left_position = position - IVec3::new(Chunk::SIZE as i32, 0, 0);
        let left_chunk = self.chunks.get(&left_position);

        let front_position = position + IVec3::new(0, 0, Chunk::SIZE as i32);
        let front_chunk = self.chunks.get(&front_position);

        let back_position = position - IVec3::new(0, 0, Chunk::SIZE as i32);
        let back_chunk = self.chunks.get(&back_position);

        // XXX: The lights default to zero to avoid having to wrap them in Option, if the
        // corresponding block is None, the light will be irrelevant.
        let mut top: [[Light; Chunk::SIZE]; Chunk::SIZE] = Default::default();
        let mut bottom: [[Light; Chunk::SIZE]; Chunk::SIZE] = Default::default();
        let mut right: [[Light; Chunk::SIZE]; Chunk::SIZE] = Default::default();
        let mut left: [[Light; Chunk::SIZE]; Chunk::SIZE] = Default::default();
        let mut front: [[Light; Chunk::SIZE]; Chunk::SIZE] = Default::default();
        let mut back: [[Light; Chunk::SIZE]; Chunk::SIZE] = Default::default();

        for i in 0..Chunk::SIZE {
            for j in 0..Chunk::SIZE {
                if let Some(top_chunk) = top_chunk {
                    top[i][j] = top_chunk[[i, 0, j]];
                }
                if let Some(bottom_chunk) = bottom_chunk {
                    bottom[i][j] = bottom_chunk[[i, Chunk::SIZE - 1, j]];
                }
                if let Some(right_chunk) = right_chunk {
                    right[i][j] = right_chunk[[0, i, j]];
                }
                if let Some(left_chunk) = left_chunk {
                    left[i][j] = left_chunk[[Chunk::SIZE - 1, i, j]];
                }
                if let Some(back_chunk) = back_chunk {
                    back[i][j] = back_chunk[[i, j, Chunk::SIZE - 1]];
                }
                if let Some(front_chunk) = front_chunk {
                    front[i][j] = front_chunk[[i, j, 0]];
                }
            }
        }

        return ExpandedLightChunk {
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

#[derive(Default, Clone, Copy, PartialEq, Eq)]
pub struct Light(pub u8);

impl std::fmt::Debug for Light {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("light")
            .field("sunlight", &self.sunlight())
            .field("artificial", &self.artificial())
            .finish()
    }
}

impl Light {
    const SUNLIGHT_MASK: u8 = 0b1111_0000;
    const ARTIFICIAL_MASK: u8 = 0b0000_1111;

    const fn new(sunlight: u8, artificial: u8) -> Self {
        Self(sunlight << 4 | artificial)
    }

    pub fn sunlight(&self) -> u8 {
        self.0 >> 4
    }

    pub fn set_sunlight(&mut self, light: u8) {
        self.0 = self.0 & Self::ARTIFICIAL_MASK | (light << 4);
    }

    pub fn artificial(&self) -> u8 {
        self.0 & Self::ARTIFICIAL_MASK
    }

    pub fn set_artificial(&mut self, light: u8) {
        self.0 = self.0 & Self::SUNLIGHT_MASK | light;
    }

    fn can_propagate(&self) -> bool {
        self.sunlight() > 1 || self.artificial() > 1
    }

    fn decrement(self, attenuation: u8) -> Self {
        self.decrement_sun(attenuation)
            .decrement_artificial(attenuation)
    }

    fn decrement_artificial(mut self, attenuation: u8) -> Self {
        let artificial = (self.0 & Self::ARTIFICIAL_MASK).saturating_sub(attenuation);
        self.0 = (self.0 & !Self::ARTIFICIAL_MASK) | artificial;
        self
    }

    fn decrement_sun(mut self, attenuation: u8) -> Self {
        let sunlight = (self.0 >> 4).saturating_sub(attenuation);
        self.0 = (self.0 & !Self::SUNLIGHT_MASK) | (sunlight << 4);
        self
    }
}

// Light from blocks and the sky are combined into one u8, 4 bits each, max 16 light levels.
#[derive(Clone)]
enum LightStorage {
    Uniform(Light),
    Normal(Vec<Light>),
}

#[derive(Clone)]
pub struct LightChunk {
    is_sunlit: bool,
    light: LightStorage,
}

impl LightChunk {
    fn new_normal() -> Self {
        Self {
            is_sunlit: false,
            light: LightStorage::Normal(vec![Light::new(0, 0); Chunk::SIZE.pow(3)]),
        }
    }

    fn new_uniform_sunlight() -> Self {
        Self {
            is_sunlit: true,
            light: LightStorage::Uniform(Light::new(15, 0)),
        }
    }

    fn new_uniform_shadow() -> Self {
        Self {
            is_sunlit: false,
            light: LightStorage::Uniform(Light::new(0, 0)),
        }
    }

    fn convert_to_normal(&mut self) {
        if matches!(self.light, LightStorage::Uniform(_)) {
            self.light = LightStorage::Normal(vec![Light::new(0, 0); Chunk::SIZE.pow(3)]);
        }
    }

    #[inline(always)]
    fn is_uniform_sunlight(&self) -> bool {
        matches!(self.light, LightStorage::Uniform(light) if light.sunlight() == 15)
    }

    fn is_uniform_shadow(&self) -> bool {
        matches!(self.light, LightStorage::Uniform(light) if light.sunlight() == 0)
    }
}

impl Index<[usize; 3]> for LightChunk {
    type Output = Light;

    fn index(&self, idx: [usize; 3]) -> &Self::Output {
        match &self.light {
            LightStorage::Uniform(light) => light,
            LightStorage::Normal(lights) => &lights[idx[0] << 8 | idx[2] << 4 | idx[1]],
        }
    }
}

impl IndexMut<[usize; 3]> for LightChunk {
    fn index_mut(&mut self, idx: [usize; 3]) -> &mut Self::Output {
        match &mut self.light {
            LightStorage::Uniform(_) => {
                panic!("Can't set the light in uniform chunks, they have to be converted first.")
            }
            LightStorage::Normal(lights) => &mut lights[idx[0] << 8 | idx[2] << 4 | idx[1]],
        }
    }
}

impl Index<usize> for LightChunk {
    type Output = Light;

    fn index(&self, idx: usize) -> &Self::Output {
        match &self.light {
            LightStorage::Uniform(light) => light,
            LightStorage::Normal(lights) => &lights[idx],
        }
    }
}

impl IndexMut<usize> for LightChunk {
    fn index_mut(&mut self, idx: usize) -> &mut Self::Output {
        match &mut self.light {
            LightStorage::Uniform(_) => {
                panic!("Can't set the light in uniform chunks, they have to be converted first.")
            }
            LightStorage::Normal(lights) => &mut lights[idx],
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
struct LightUpdate {
    index: usize,
    light: Light,
}

impl Ord for LightUpdate {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (self.light.artificial() + self.light.sunlight())
            .cmp(&(other.light.artificial() + other.light.sunlight()))
    }
}

impl PartialOrd for LightUpdate {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Resource, Default, DerefMut, Deref)]
struct Queues(HashMap<IVec3, LightUpdateQueue>);

const QUEUE_DELAY: std::time::Duration = std::time::Duration::from_secs(5);

struct LightUpdateQueue {
    // When sunlight gets propagated into a chunk from one of its adjacent chunks(i.e. not from above) a
    // timer is added to the propagation queue. The queue will not be processed before it either
    // expires, or sunlight is received from above. This is to ensure that the direct sunlight gets
    // processed first, as processing the sunlight from the adjacent chunks first will cause upwards of
    // 10-20k lighting updates before it will settle.
    sunlit: bool,
    timer: std::time::Instant,
    // This is a ring buffer to allow for prioritization of propagations. Direct sunlight for
    // example will be put at the end of the queue to have it processed first.
    propagation: VecDeque<LightUpdate>,
    removal: BinaryHeap<LightUpdate>,
}

impl LightUpdateQueue {
    fn new() -> Self {
        Self {
            sunlit: false,
            timer: std::time::Instant::now().checked_sub(QUEUE_DELAY).unwrap(),
            propagation: VecDeque::with_capacity(Chunk::SIZE.pow(2) * 6),
            removal: BinaryHeap::new(),
        }
    }

    fn add_timer(&mut self) {
        self.timer = std::time::Instant::now();
    }
}

fn handle_new_chunks(
    mut light_map: ResMut<LightMap>,
    world_map: Res<WorldMap>,
    mut light_update_queues: ResMut<Queues>,
    mut new_chunks: EventReader<NewChunkEvent>,
) {
    let blocks = Blocks::get();

    for new_chunk in new_chunks.read() {
        if light_map.chunks.contains_key(&new_chunk.position) {
            // There may be duplicate events, ignore them.
            continue;
        };

        let Some(chunk) = world_map.get_chunk(&new_chunk.position) else {
            continue;
        };

        // All uniform chunks where sunlight can travel unimpeded are counted as origins of
        // sunlight as long as they are 4 chunks above y=0.
        let light_chunk = if chunk.is_uniform()
            && blocks.get_config(chunk[0]).light_attenuation() == 0
            && new_chunk.position.y >= 64
        {
            // TODO: This is not accurate. The server can know where there is sunlight, and it will
            // probably need it too in some way.
            LightChunk::new_uniform_sunlight()
        } else {
            let mut light_chunk = LightChunk::new_uniform_shadow();
            let mut light_update_queue = LightUpdateQueue::new();

            for (index, block_id) in chunk.iter_blocks() {
                let block_config = blocks.get_config(*block_id);
                let light = block_config.light_level();
                if light > 1 {
                    light_update_queue.propagation.push_back(LightUpdate {
                        index,
                        light: Light::new(0, light),
                    });
                }
            }

            for chunk_face in [
                ChunkFace::Top,
                ChunkFace::Bottom,
                ChunkFace::Right,
                ChunkFace::Left,
                ChunkFace::Front,
                ChunkFace::Back,
            ] {
                let adjacent_chunk_position = chunk_face.shift_position(new_chunk.position);

                let Some(adjacent_light_chunk) = light_map.chunks.get(&adjacent_chunk_position)
                else {
                    continue;
                };

                if adjacent_light_chunk.is_uniform_shadow() {
                    continue;
                } else if chunk_face == ChunkFace::Top
                    && adjacent_light_chunk.is_uniform_sunlight()
                    && chunk.is_uniform()
                {
                    light_chunk = LightChunk::new_uniform_sunlight();
                    break;
                }

                if adjacent_light_chunk.is_sunlit {
                    if chunk_face == ChunkFace::Top {
                        light_update_queue.sunlit = true;
                    }
                    light_update_queue.add_timer();
                }

                for i in 0..Chunk::SIZE {
                    for j in 0..Chunk::SIZE {
                        let (index, adjacent_index) = match chunk_face {
                            // TODO: All this bitshifting is confusing, use one of the utility
                            // functions for position perhaps. Or just push ivecs for updates idk.
                            ChunkFace::Top => {
                                (i << 8 | j << 4 | (Chunk::SIZE - 1), i << 8 | j << 4)
                            }
                            ChunkFace::Bottom => {
                                (i << 8 | j << 4, i << 8 | j << 4 | (Chunk::SIZE - 1))
                            }
                            ChunkFace::Right => ((Chunk::SIZE - 1) << 8 | i << 4 | j, i << 4 | j),
                            ChunkFace::Left => (i << 4 | j, (Chunk::SIZE - 1) << 8 | i << 4 | j),
                            ChunkFace::Front => (i << 8 | (Chunk::SIZE - 1) << 4 | j, i << 8 | j),
                            ChunkFace::Back => (i << 8 | j, i << 8 | (Chunk::SIZE - 1) << 4 | j),
                            _ => unreachable!(),
                        };

                        let adjacent_light = adjacent_light_chunk[adjacent_index];

                        if !adjacent_light.can_propagate() {
                            continue;
                        }

                        // Attenuation is 0 when it's sunlight, 1 otherwise
                        let attenuation =
                            (chunk_face != ChunkFace::Top || adjacent_light.sunlight() != 15) as u8;
                        let light = adjacent_light.decrement(attenuation);

                        if light.sunlight() == 15 {
                            light_update_queue
                                .propagation
                                .push_back(LightUpdate { index, light });
                        } else {
                            light_update_queue
                                .propagation
                                .push_front(LightUpdate { index, light });
                        }
                    }
                }
            }

            if !light_chunk.is_uniform_sunlight() {
                light_update_queues.insert(new_chunk.position, light_update_queue);
            }
            light_chunk
        };

        let is_uniform_sunlight = light_chunk.is_uniform_sunlight();
        light_map.chunks.insert(new_chunk.position, light_chunk);

        // Transfer sunlight to all connected uniform shadow chunks below
        if is_uniform_sunlight {
            light_map.propagate_to_adjacent(
                new_chunk.position,
                &mut light_update_queues,
                chunk,
                blocks,
            );
            let mut chunk_position = new_chunk.position;
            chunk_position.y -= Chunk::SIZE as i32;
            while let Some(light_chunk) = light_map.chunks.get_mut(&chunk_position) {
                let Some(chunk) = world_map.get_chunk(&chunk_position) else {
                    break;
                };

                if light_chunk.is_uniform_shadow() && chunk.is_uniform() {
                    *light_chunk = LightChunk::new_uniform_sunlight();
                    light_map.propagate_to_adjacent(
                        chunk_position,
                        &mut light_update_queues,
                        chunk,
                        blocks,
                    );
                } else {
                    break;
                }

                chunk_position.y -= Chunk::SIZE as i32;
            }
        }
    }
}

fn handle_block_updates(
    mut light_map: ResMut<LightMap>,
    mut light_update_queues: ResMut<Queues>,
    mut block_updates_events: EventReader<messages::BlockUpdates>,
) {
    let blocks = Blocks::get();

    for block_updates in block_updates_events.read() {
        let Some(mut light_chunk) = light_map.chunks.remove(&block_updates.chunk_position) else {
            continue;
        };
        for (index, block_id, _) in block_updates.blocks.iter() {
            let light = match &mut light_chunk.light {
                LightStorage::Uniform(uniform_light) => {
                    if uniform_light.sunlight() != 0 {
                        light_chunk.light =
                            LightStorage::Normal(vec![*uniform_light; Chunk::SIZE.pow(3)]);
                        &mut light_chunk[*index]
                    } else {
                        continue;
                    }
                }
                LightStorage::Normal(light_chunk) => &mut light_chunk[*index],
            };

            if !blocks.contains(*block_id) {
                // Non-existent blocks are handled when changing the blocks in the chunk, so we ignore.
                continue;
            }

            let queue = light_update_queues
                .entry(block_updates.chunk_position)
                .or_insert(LightUpdateQueue::new());

            let block_config = blocks.get_config(*block_id);
            if block_config.light_level() > 0 {
                queue.propagation.push_front(LightUpdate {
                    index: *index,
                    light: Light::new(0, block_config.light_level()),
                });
            }

            // In the event that the light is sunlight = 0, artificial = 0, the removal would
            // do nothing, so we substitute a false light level.
            if *light == Light::new(0, 0) {
                *light = Light::new(1, 1);
            }

            queue.removal.push(LightUpdate {
                index: *index,
                light: *light,
            });

            // for block_offset in [
            //     IVec3::X,
            //     IVec3::NEG_X,
            //     IVec3::Z,
            //     IVec3::NEG_Z,
            //     IVec3::Y,
            //     IVec3::NEG_Y,
            // ] {
            //     let (chunk_offset, index) = utils::world_position_to_chunk_position_and_block_index(
            //         utils::block_index_to_position(*index) + block_offset,
            //     );
            //     let chunk_position = block_updates.chunk_position + chunk_offset;
            //
            //     let update_queue = if let Some(adj) = light_update_queues.get_mut(&chunk_position) {
            //         adj
            //     } else if light_map.chunks.contains_key(&chunk_position)
            //         || chunk_position == block_updates.chunk_position
            //     {
            //         light_update_queues
            //             .entry(chunk_position)
            //             .or_insert(LightUpdateQueue::new())
            //     } else {
            //         continue;
            //     };
            //
            //     update_queue.removal.push_back(LightUpdate {
            //         index,
            //         light: light
            //             .decrement_sun(
            //                 (light.sunlight() != 15 || block_offset != IVec3::NEG_Y) as u8,
            //             )
            //             .decrement_artificial(1),
            //     });
            // }
            //
            // *light = Light::new(0, 0);
        }

        light_map
            .chunks
            .insert(block_updates.chunk_position, light_chunk);
    }
}

fn propagate_light(
    world_map: Res<WorldMap>,
    mut light_update_queues: ResMut<Queues>,
    mut light_map: ResMut<LightMap>,
    mut chunk_mesh_events: EventWriter<TestFinishedLightingEvent>,
) {
    let blocks = Blocks::get();

    for chunk_position in light_update_queues.keys().cloned().collect::<Vec<IVec3>>() {
        let mut update_queue = light_update_queues.remove(&chunk_position).unwrap();

        let Some(mut light_chunk) = light_map.chunks.remove(&chunk_position) else {
            continue;
        };

        if update_queue.timer.elapsed() < QUEUE_DELAY
            && !(light_chunk.is_sunlit || update_queue.sunlit)
        {
            light_map.chunks.insert(chunk_position, light_chunk);
            light_update_queues.insert(chunk_position, update_queue);
            continue;
        }

        let Some(chunk) = world_map.get_chunk(&chunk_position) else {
            // XXX: Notice it also drops the light chunk
            continue;
        };

        while let Some(removal) = update_queue.removal.pop() {
            let light = match &mut light_chunk.light {
                LightStorage::Uniform(uniform_light) => {
                    if uniform_light.sunlight() != 0 {
                        light_chunk.light =
                            LightStorage::Normal(vec![*uniform_light; Chunk::SIZE.pow(3)]);
                        &mut light_chunk[removal.index]
                    } else {
                        continue;
                    }
                }
                LightStorage::Normal(light_chunk) => &mut light_chunk[removal.index],
            };

            let mut removed_light = Light::new(0, 0);

            if light.sunlight() != 0 && light.sunlight() <= removal.light.sunlight() {
                removed_light.set_sunlight(light.sunlight());
                light.set_sunlight(0);
            }

            if light.artificial() != 0 && light.artificial() <= removal.light.artificial() {
                removed_light.set_artificial(light.artificial());
                light.set_artificial(0);
            }

            let attenuation = blocks.get_config(chunk[removal.index]).light_attenuation();

            if removed_light != Light::new(0, 0) {
                for block_offset in [
                    IVec3::X,
                    IVec3::NEG_X,
                    IVec3::Z,
                    IVec3::NEG_Z,
                    IVec3::Y,
                    IVec3::NEG_Y,
                ] {
                    let (chunk_offset, index) =
                        utils::world_position_to_chunk_position_and_block_index(
                            utils::block_index_to_position(removal.index) + block_offset,
                        );
                    let adjacent_chunk_position = chunk_position + chunk_offset;

                    let update_queue = if adjacent_chunk_position != chunk_position {
                        if let Some(adj) = light_update_queues.get_mut(&adjacent_chunk_position) {
                            adj
                        } else if light_map.chunks.contains_key(&adjacent_chunk_position) {
                            light_update_queues
                                .entry(adjacent_chunk_position)
                                .or_insert(LightUpdateQueue::new())
                        } else {
                            continue;
                        }
                    } else {
                        &mut update_queue
                    };

                    update_queue.removal.push(LightUpdate {
                        index,
                        light: removed_light
                            .decrement_sun(
                                (removed_light.sunlight() != 15 || block_offset != IVec3::NEG_Y)
                                    as u8,
                            )
                            .decrement_artificial(1),
                    });
                }
            } else if light.decrement(attenuation.max(1)).can_propagate() {
                for block_offset in [
                    IVec3::NEG_Y,
                    IVec3::Y,
                    IVec3::X,
                    IVec3::NEG_X,
                    IVec3::Z,
                    IVec3::NEG_Z,
                ] {
                    let (chunk_offset, index) =
                        utils::world_position_to_chunk_position_and_block_index(
                            utils::block_index_to_position(removal.index) + block_offset,
                        );
                    let adjacent_chunk_position = chunk_position + chunk_offset;

                    let update_queue = if adjacent_chunk_position != chunk_position {
                        if let Some(adj) = light_update_queues.get_mut(&adjacent_chunk_position) {
                            adj
                        } else if light_map.chunks.contains_key(&adjacent_chunk_position) {
                            light_update_queues
                                .entry(adjacent_chunk_position)
                                .or_insert(LightUpdateQueue::new())
                        } else {
                            continue;
                        }
                    } else {
                        &mut update_queue
                    };

                    update_queue.propagation.push_front(LightUpdate {
                        index,
                        light: light
                            .decrement_sun(u8::max(
                                (light.sunlight() != 15 || block_offset != IVec3::NEG_Y) as u8,
                                attenuation,
                            ))
                            .decrement_artificial(attenuation.max(1)),
                    });
                }
            }
        }

        // The initial sunlight propagation
        if update_queue.propagation.len() > 0 && update_queue.sunlit {
            // TODO: Maybe do a two-layered height map? First height is the block where it was
            // first attenuated, and second the block it became light level 0. This way water can
            // also be sunlit quickly.
            //
            // un-attenuated sunlight is spread differently. Normal spreading is somewhat
            // expensive; and there's a lot of sunlight. A "ray" is spread all the way to the ground,
            // the height one above where it collides is stored. Adjacent values in the height map
            // can be used to determine if there's a need to spread the sunlight sideways. This way
            // we save most of the redundant sideways propagations, and spreading to adjacent
            // chunks can be batched.
            let mut height_map: [usize; Chunk::SIZE.pow(2)] = [Chunk::SIZE; Chunk::SIZE.pow(2)];

            light_chunk.convert_to_normal();

            while let Some(propagation) = update_queue.propagation.pop_back() {
                if propagation.light.sunlight() != 15 {
                    update_queue.propagation.push_back(propagation);

                    break;
                }

                // Propagate the sunlight downward
                let max_y = propagation.index & 0b1111;
                let mut index = Chunk::SIZE - 1;
                for y in 0..=max_y {
                    index = propagation.index - y;

                    light_chunk[index].set_sunlight(propagation.light.sunlight());

                    let attenuation = blocks.get_config(chunk[index]).light_attenuation();
                    if attenuation != 0 {
                        break;
                    }
                }
                height_map[index >> 4] = index & 0b1111;
            }

            for x in 0..Chunk::SIZE {
                for z in 0..Chunk::SIZE {
                    let stop = height_map[x << 4 | z];
                    // Vertical propagation stops as soon as the light level is no longer 15, so
                    // propagate once down if it stopped early.
                    if stop > 0 {
                        let index = x << 8 | z << 4 | stop;
                        let attenuation = blocks.get_config(chunk[index]).light_attenuation();

                        if light_chunk[index].decrement(attenuation).can_propagate() {
                            update_queue.propagation.push_front(LightUpdate {
                                index: index - 1,
                                light: light_chunk[index].decrement(attenuation),
                            });
                        }
                    }

                    for (shift_x, shift_z) in [(1, 0), (0, 1), (-1, 0), (0, -1)] {
                        let shifted_x = (x as i32 + shift_x) as usize;
                        let shifted_z = (z as i32 + shift_z) as usize;
                        // If the shifted position is outside the 0..CHUNK_SIZE range, it is
                        // outside the chunk.
                        if shifted_x >= Chunk::SIZE || shifted_z >= Chunk::SIZE {
                            // We go from i32 to usize so negative values get underflowed
                            continue;
                        }

                        let from = height_map[shifted_x << 4 | shifted_z];

                        for y in (from..stop).rev() {
                            let attenuation = blocks
                                .get_config(chunk[[shifted_x, y, shifted_z]])
                                .light_attenuation()
                                .max(1);
                            let light = light_chunk[[shifted_x, y, shifted_z]];
                            if !light.decrement(attenuation).can_propagate() {
                                break;
                            }
                            update_queue.propagation.push_back(LightUpdate {
                                index: x << 8 | z << 4 | y,
                                light: light.decrement(attenuation),
                            });
                        }
                    }
                }
            }
        }

        while let Some(propagation) = update_queue.propagation.pop_back() {
            let light = match &mut light_chunk.light {
                LightStorage::Uniform(uniform_light) => {
                    if propagation.light.sunlight() > uniform_light.sunlight()
                        || propagation.light.artificial() > 0
                    {
                        light_chunk.light =
                            LightStorage::Normal(vec![*uniform_light; Chunk::SIZE.pow(3)]);
                        &mut light_chunk[propagation.index]
                    } else {
                        continue;
                    }
                }
                LightStorage::Normal(light_chunk) => &mut light_chunk[propagation.index],
            };

            let mut changed = false;

            if propagation.light.sunlight() > light.sunlight() {
                light.set_sunlight(propagation.light.sunlight());
                changed = true;
            }

            if propagation.light.artificial() > light.artificial() {
                light.set_artificial(propagation.light.artificial());
                changed = true;
            }

            let mut attenuation = blocks
                .get_config(chunk[propagation.index])
                .light_attenuation();

            if propagation.light.sunlight() != 15 {
                // All light is always pre-attenuated by 1 when propagated unless it is direct
                // sunlight(15 moving down). This is to differentiate direct sunlight from the
                // rest. Subtract 1 from the attenuation to compensate.
                attenuation = attenuation.saturating_sub(1);
            }

            if !changed || !light.decrement(attenuation).can_propagate() {
                continue;
            }

            for block_offset in [
                IVec3::NEG_Y,
                IVec3::Y,
                IVec3::X,
                IVec3::NEG_X,
                IVec3::Z,
                IVec3::NEG_Z,
            ] {
                let (chunk_offset, index) = utils::world_position_to_chunk_position_and_block_index(
                    utils::block_index_to_position(propagation.index) + block_offset,
                );
                let adjacent_chunk_position = chunk_position + chunk_offset;
                let update_queue = if adjacent_chunk_position != chunk_position {
                    if update_queue.sunlit {
                        // Skip, send to adjacent chunks in batch later.
                        continue;
                    }

                    if let Some(adj) = light_update_queues.get_mut(&adjacent_chunk_position) {
                        adj
                    } else if light_map.chunks.contains_key(&adjacent_chunk_position) {
                        light_update_queues
                            .entry(adjacent_chunk_position)
                            .or_insert(LightUpdateQueue::new())
                    } else {
                        continue;
                    }
                } else {
                    &mut update_queue
                };

                update_queue.propagation.push_front(LightUpdate {
                    index,
                    light: light
                        .decrement_sun(u8::max(
                            (light.sunlight() != 15 || block_offset != IVec3::NEG_Y) as u8,
                            attenuation,
                        ))
                        .decrement_artificial(attenuation.max(1)),
                });
            }
        }

        if !light_chunk.is_sunlit && update_queue.sunlit {
            light_chunk.is_sunlit = true;
            light_map.chunks.insert(chunk_position, light_chunk);
            light_map.propagate_to_adjacent(
                chunk_position,
                &mut light_update_queues,
                chunk,
                blocks,
            );
        } else {
            light_map.chunks.insert(chunk_position, light_chunk);
        }
        light_update_queues.insert(chunk_position, update_queue);
    }

    light_update_queues.retain(|chunk_position, queue| {
        if queue.propagation.is_empty() && queue.removal.is_empty() {
            chunk_mesh_events.write(TestFinishedLightingEvent(*chunk_position));
            chunk_mesh_events.write(TestFinishedLightingEvent(
                *chunk_position + IVec3::new(Chunk::SIZE as i32, 0, 0),
            ));
            chunk_mesh_events.write(TestFinishedLightingEvent(
                *chunk_position - IVec3::new(Chunk::SIZE as i32, 0, 0),
            ));
            chunk_mesh_events.write(TestFinishedLightingEvent(
                *chunk_position + IVec3::new(0, Chunk::SIZE as i32, 0),
            ));
            chunk_mesh_events.write(TestFinishedLightingEvent(
                *chunk_position - IVec3::new(0, Chunk::SIZE as i32, 0),
            ));
            chunk_mesh_events.write(TestFinishedLightingEvent(
                *chunk_position + IVec3::new(0, 0, Chunk::SIZE as i32),
            ));
            chunk_mesh_events.write(TestFinishedLightingEvent(
                *chunk_position - IVec3::new(0, 0, Chunk::SIZE as i32),
            ));
            false
        } else {
            true
        }
    });
}

fn light_chunk_unloading(world_map: Res<WorldMap>, mut light_map: ResMut<LightMap>) {
    for position in light_map.chunks.keys().cloned().collect::<Vec<_>>().iter() {
        if !world_map.contains_chunk(position) {
            light_map.chunks.remove(position);
        }
    }
}

#[derive(Event, Hash, PartialEq, Eq)]
struct TestFinishedLightingEvent(IVec3);

// TODO: Don't rebuild surrounding chunks unless a block at the edge of the chunk has changed.
fn send_chunk_mesh_events(
    light_map: Res<LightMap>,
    light_update_queues: Res<Queues>,
    mut lighting_events: EventReader<TestFinishedLightingEvent>,
    mut chunk_mesh_events: EventWriter<ChunkMeshEvent>,
) {
    // Multiple events are often sent so duplicates are removed through the hashset
    for light_event in lighting_events
        .read()
        .collect::<HashSet<&TestFinishedLightingEvent>>()
    {
        let position = light_event.0;
        if light_map.chunks.contains_key(&position)
            && !light_update_queues.contains_key(&position)
            && !light_update_queues.contains_key(&(position + IVec3::new(0, Chunk::SIZE as i32, 0)))
            && !light_update_queues.contains_key(&(position - IVec3::new(0, Chunk::SIZE as i32, 0)))
            && !light_update_queues.contains_key(&(position + IVec3::new(Chunk::SIZE as i32, 0, 0)))
            && !light_update_queues.contains_key(&(position - IVec3::new(Chunk::SIZE as i32, 0, 0)))
            && !light_update_queues.contains_key(&(position + IVec3::new(0, 0, Chunk::SIZE as i32)))
            && !light_update_queues.contains_key(&(position - IVec3::new(0, 0, Chunk::SIZE as i32)))
        {
            chunk_mesh_events.write(ChunkMeshEvent {
                chunk_position: position,
            });
        }
    }
}
