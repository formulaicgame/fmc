use std::{
    collections::{BinaryHeap, HashMap, VecDeque},
    ops::{Index, IndexMut},
};

use bevy::prelude::*;
use fmc_protocol::messages;

use crate::{
    blocks::Blocks,
    game_state::GameState,
    settings::Settings,
    world::{BlockPosition, Chunk, ChunkFace, ChunkPosition, NewChunkEvent, Origin, WorldMap},
};

use super::{
    RenderSet,
    light_texture::{LightTexture, LightTextureUploads, LightUpload},
};

pub struct LightingPlugin;
impl Plugin for LightingPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(LightMap::default())
            .insert_resource(PropagtionQueues::default())
            .add_systems(
                Update,
                (
                    // This is done first so any removals are processed before handle_new_chunks
                    // pulls from adjacent chunks. The pull would be stale if handle_block_updates
                    // ran after it and removed the light it pulled.
                    handle_block_updates,
                    handle_new_chunks.after(handle_block_updates),
                    propagate_light.after(handle_new_chunks),
                    light_chunk_unloading.run_if(resource_changed::<Origin>),
                )
                    .in_set(RenderSet::Light)
                    .run_if(in_state(GameState::Playing)),
            )
            .add_systems(
                Update,
                reupload_light_texture.run_if(resource_exists_and_changed::<LightTexture>),
            )
            .add_systems(OnEnter(GameState::Launcher), cleanup);
    }
}

fn cleanup(mut light_map: ResMut<LightMap>) {
    light_map.chunks.clear();
}

#[derive(Resource, Default)]
pub struct LightMap {
    chunks: HashMap<ChunkPosition, LightChunk>,
}

impl LightMap {
    fn pull_from_adjacent(
        &self,
        chunk_position: ChunkPosition,
        queue: &mut LightUpdateQueue,
        world_map: &WorldMap,
        blocks: &Blocks,
    ) {
        for chunk_face in [
            ChunkFace::Top,
            ChunkFace::Bottom,
            ChunkFace::Right,
            ChunkFace::Left,
            ChunkFace::Front,
            ChunkFace::Back,
        ] {
            let Some(light_chunk) = self.chunks.get(&chunk_position) else {
                continue;
            };

            let adjacent_chunk_position = chunk_position.offset(chunk_face);
            let Some(adjacent_light_chunk) = self.chunks.get(&adjacent_chunk_position) else {
                continue;
            };

            if adjacent_light_chunk.is_uniform_shadow() {
                continue;
            }

            let Some(adjacent_chunk) = world_map.get_chunk(&adjacent_chunk_position) else {
                continue;
            };

            if chunk_face == ChunkFace::Top
                && adjacent_light_chunk.is_sunlit
                && !light_chunk.is_sunlit
            {
                queue.sunlight_pass = true;
            }

            for i in 0..Chunk::SIZE {
                for j in 0..Chunk::SIZE {
                    let (index, adjacent_index) = match chunk_face {
                        // TODO: All this bitshifting is confusing, use one of the utility
                        // functions for position perhaps. Or just push ivecs for updates idk.
                        ChunkFace::Top => (i << 8 | j << 4 | (Chunk::SIZE - 1), i << 8 | j << 4),
                        ChunkFace::Bottom => (i << 8 | j << 4, i << 8 | j << 4 | (Chunk::SIZE - 1)),
                        ChunkFace::Right => ((Chunk::SIZE - 1) << 8 | i << 4 | j, i << 4 | j),
                        ChunkFace::Left => (i << 4 | j, (Chunk::SIZE - 1) << 8 | i << 4 | j),
                        ChunkFace::Front => (i << 8 | (Chunk::SIZE - 1) << 4 | j, i << 8 | j),
                        ChunkFace::Back => (i << 8 | j, i << 8 | (Chunk::SIZE - 1) << 4 | j),
                        _ => unreachable!(),
                    };

                    let light = light_chunk[index];
                    let adjacent_light = adjacent_light_chunk[adjacent_index];
                    let adjacent_attenuation = blocks
                        .get_config(adjacent_chunk[adjacent_index])
                        .light_attenuation();
                    let new_light = adjacent_light
                        .attenuate(adjacent_attenuation, chunk_face == ChunkFace::Top);

                    if light.sunlight() >= new_light.sunlight()
                        && light.artificial() >= new_light.artificial()
                    {
                        continue;
                    }

                    let update = LightUpdate {
                        index,
                        light: new_light,
                    };
                    if chunk_face == ChunkFace::Top {
                        queue.propagation.push_back(update);
                    } else {
                        queue.propagation.push_front(update);
                    }
                }
            }

            if queue.sunlight_pass {
                // The sunlight pass must have been completed before we can pull in light
                // updates from adjacent chunks.
                // If this wasn't done the horizontal spreading of sunlight would take up an
                // inordinate amount of time, just to be replaced immediately.
                // If the above chunk isn't sunlit though, we include any adjacent chunk's light as
                // long as it isn't sunlit itself, this takes care of any spreading where direct
                // sunlight doesn't reach.
                return;
            }
        }
    }

    fn push_to_adjacent(
        &mut self,
        chunk_position: ChunkPosition,
        light_update_queues: &mut PropagtionQueues,
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

            let adjacent_chunk_position = chunk_position.offset(chunk_face);
            let Some(adjacent_light_chunk) = self.chunks.get(&adjacent_chunk_position) else {
                continue;
            };

            if light_chunk.is_uniform_shadow()
                || (light_chunk.is_uniform_sunlight() && adjacent_light_chunk.is_uniform_sunlight())
            {
                continue;
            }

            let adjacent_light_queue = light_update_queues
                .entry(adjacent_chunk_position)
                .or_insert_with(LightUpdateQueue::new);

            if chunk_face == ChunkFace::Bottom && light_chunk.is_sunlit {
                adjacent_light_queue.sunlight_pass = true;
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

                    let attenuation = blocks.get_config(chunk[index]).light_attenuation();
                    let light =
                        light_chunk[index].attenuate(attenuation, chunk_face == ChunkFace::Bottom);

                    if light == Light::NONE {
                        continue;
                    }

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
}

#[derive(Default, Clone, Copy, PartialEq, Eq)]
struct Light(pub u8);

impl std::fmt::Debug for Light {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("light")
            .field("sunlight", &self.sunlight())
            .field("artificial", &self.artificial())
            .finish()
    }
}

impl Light {
    const NONE: Light = Light::new(0, 0);
    const SUNLIGHT_MASK: u8 = 0b1111_0000;
    const ARTIFICIAL_MASK: u8 = 0b0000_1111;

    const fn new(sunlight: u8, artificial: u8) -> Self {
        Self(sunlight << 4 | artificial)
    }

    fn sunlight(&self) -> u8 {
        self.0 >> 4
    }

    fn set_sunlight(&mut self, light: u8) {
        self.0 = self.0 & Self::ARTIFICIAL_MASK | (light << 4);
    }

    fn artificial(&self) -> u8 {
        self.0 & Self::ARTIFICIAL_MASK
    }

    fn set_artificial(&mut self, light: u8) {
        self.0 = self.0 & Self::SUNLIGHT_MASK | light;
    }

    fn attenuate(self, attenuation: u8, downward: bool) -> Self {
        let sun_attenuation = if downward && self.sunlight() == 15 {
            attenuation
        } else {
            attenuation.max(1)
        };
        self.decrement_sun(sun_attenuation)
            .decrement_artificial(attenuation.max(1))
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
struct LightChunk {
    is_sunlit: bool,
    light: LightStorage,
}

impl LightChunk {
    fn new_uniform_sunlight() -> Self {
        Self {
            is_sunlit: true,
            light: LightStorage::Uniform(Light::new(15, 0)),
        }
    }

    fn new_uniform_shadow() -> Self {
        Self {
            is_sunlit: false,
            light: LightStorage::Uniform(Light::NONE),
        }
    }

    fn convert_to_normal(&mut self) {
        if let LightStorage::Uniform(uniform_light) = self.light {
            self.light = LightStorage::Normal(vec![uniform_light; Chunk::SIZE.pow(3)]);
        }
    }

    #[inline(always)]
    fn is_uniform_sunlight(&self) -> bool {
        matches!(self.light, LightStorage::Uniform(light) if light.sunlight() == 15)
    }

    fn is_uniform_shadow(&self) -> bool {
        matches!(self.light, LightStorage::Uniform(light) if light.sunlight() == 0)
    }

    fn as_upload(&self) -> LightUpload {
        match &self.light {
            LightStorage::Uniform(light) => {
                if light.sunlight() == 15 {
                    LightUpload::uniform_sunlight()
                } else {
                    LightUpload::uniform_dark()
                }
            }
            LightStorage::Normal(lights) => {
                let mut data = [0; Chunk::SIZE.pow(3) * 2];
                for x in 0..Chunk::SIZE {
                    for z in 0..Chunk::SIZE {
                        for y in 0..Chunk::SIZE {
                            let light = lights[x << 8 | z << 4 | y];
                            let index = (z * Chunk::SIZE.pow(2) + y * Chunk::SIZE + x) * 2;
                            // Scaled so 15 maps to 1.0 in the shader
                            data[index] = light.sunlight() * 17;
                            data[index + 1] = light.artificial() * 17;
                        }
                    }
                }
                LightUpload::new(data)
            }
        }
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
struct PropagtionQueues(HashMap<ChunkPosition, LightUpdateQueue>);

struct LightUpdateQueue {
    // When sunlight gets propagated into a chunk from one of its adjacent chunks(i.e. not from above) a
    // timer is added to the queue. The queue will not be processed before it either
    // expires, or sunlight is received from above. This is to ensure that the direct sunlight gets
    // processed first, as processing the sunlight from the adjacent chunks first will be very
    // expensive.
    sunlight_pass: bool,
    // This is a ring buffer to allow for prioritization of propagations. Direct sunlight for
    // example will be put at the end of the queue to have it processed first.
    propagation: VecDeque<LightUpdate>,
}

impl LightUpdateQueue {
    fn new() -> Self {
        Self {
            sunlight_pass: false,
            propagation: VecDeque::with_capacity(Chunk::SIZE.pow(2) * 6),
        }
    }
}

fn handle_new_chunks(
    mut light_map: ResMut<LightMap>,
    world_map: Res<WorldMap>,
    blocks: Res<Blocks>,
    mut light_update_queues: ResMut<PropagtionQueues>,
    mut new_chunks: MessageReader<NewChunkEvent>,
) {
    for new_chunk in new_chunks.read() {
        if light_map.chunks.contains_key(&new_chunk.position) {
            // There may be duplicate events, ignore them.
            continue;
        };

        let Some(chunk) = world_map.get_chunk(&new_chunk.position) else {
            continue;
        };

        let mut light_update_queue = LightUpdateQueue::new();

        // All uniform chunks where sunlight can travel unimpeded are counted as origins of
        // sunlight as long as they aren't below y=0.
        if chunk.is_uniform()
            && blocks.get_config(chunk[0]).light_attenuation() == 0
            && new_chunk.position.y >= 0
        {
            light_map
                .chunks
                .insert(new_chunk.position, LightChunk::new_uniform_sunlight());

            light_map.push_to_adjacent(
                new_chunk.position,
                &mut light_update_queues,
                chunk,
                &blocks,
            );
        } else {
            light_map
                .chunks
                .insert(new_chunk.position, LightChunk::new_uniform_shadow());

            light_map.pull_from_adjacent(
                new_chunk.position,
                &mut light_update_queue,
                &world_map,
                &blocks,
            );
        };

        if !chunk.is_uniform() {
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
        }

        // The queue might be empty, but it serves the dual purpose of triggering an update to the
        // lighting texture.
        light_update_queues.insert(new_chunk.position, light_update_queue);
    }
}

fn handle_block_updates(
    world_map: Res<WorldMap>,
    blocks: Res<Blocks>,
    mut light_map: ResMut<LightMap>,
    mut light_update_queues: ResMut<PropagtionQueues>,
    mut block_updates_events: MessageReader<messages::BlockUpdates>,
    mut removal_queues: Local<HashMap<ChunkPosition, BinaryHeap<LightUpdate>>>,
) {
    for block_updates in block_updates_events.read() {
        let chunk_position = ChunkPosition::from(block_updates.chunk_position);
        let Some(mut light_chunk) = light_map.chunks.remove(&chunk_position) else {
            continue;
        };
        for (index, block_id, _) in block_updates.blocks.iter() {
            let light = match &mut light_chunk.light {
                LightStorage::Uniform(uniform_light) => {
                    // TODO: It needs this to light properly, but it means it won't light up
                    // anything underground when you place an artificial light there. Artificial
                    // lighting updates must be separated out I think, it's turned into a mess
                    // anyway.
                    // if uniform_light.sunlight() != 0 {
                    light_chunk.light =
                        LightStorage::Normal(vec![*uniform_light; Chunk::SIZE.pow(3)]);
                    &mut light_chunk[*index]
                    // } else {
                    //     continue;
                    // }
                }
                LightStorage::Normal(light_chunk) => &mut light_chunk[*index],
            };

            if !blocks.contains(*block_id) {
                // Non-existent blocks are handled when changing the blocks in the chunk, so we ignore.
                continue;
            }

            let queue = removal_queues.entry(chunk_position).or_default();

            let block_config = blocks.get_config(*block_id);
            if block_config.light_level() > 0 {
                queue.push(LightUpdate {
                    index: *index,
                    light: Light::new(0, block_config.light_level()),
                });
            }

            // In the event that the light is sunlight = 0, artificial = 0, the removal would
            // do nothing, so we substitute a false light level.
            if *light == Light::NONE {
                *light = Light::new(1, 1);
            }

            queue.push(LightUpdate {
                index: *index,
                light: *light,
            });
        }

        light_map.chunks.insert(chunk_position, light_chunk);
    }

    while let Some(chunk_position) = removal_queues.keys().next().copied() {
        let mut removal_queue = removal_queues.remove(&chunk_position).unwrap();

        let Some(chunk) = world_map.get_chunk(&chunk_position) else {
            continue;
        };

        let Some(mut light_chunk) = light_map.chunks.remove(&chunk_position) else {
            continue;
        };

        let mut update_queue = light_update_queues
            .remove(&chunk_position)
            .unwrap_or_else(LightUpdateQueue::new);

        while let Some(removal) = removal_queue.pop() {
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

            let mut removed_light = Light::NONE;

            if light.sunlight() != 0 && light.sunlight() <= removal.light.sunlight() {
                removed_light.set_sunlight(light.sunlight());
                light.set_sunlight(0);
            }

            if light.artificial() != 0 && light.artificial() <= removal.light.artificial() {
                removed_light.set_artificial(light.artificial());
                light.set_artificial(0);
            }

            let attenuation = blocks.get_config(chunk[removal.index]).light_attenuation();

            if removed_light != Light::NONE {
                for block_offset in [
                    IVec3::X,
                    IVec3::NEG_X,
                    IVec3::Z,
                    IVec3::NEG_Z,
                    IVec3::Y,
                    IVec3::NEG_Y,
                ] {
                    let block_position = BlockPosition::from(removal.index) + block_offset;
                    let index = block_position.as_chunk_index();
                    let adjacent_chunk_position =
                        chunk_position + ChunkPosition::from(block_position);

                    let removal_update = LightUpdate {
                        index,
                        light: removed_light
                            .decrement_sun(
                                (removed_light.sunlight() != 15 || block_offset != IVec3::NEG_Y)
                                    as u8,
                            )
                            .decrement_artificial(1),
                    };

                    if adjacent_chunk_position == chunk_position {
                        removal_queue.push(removal_update);
                    } else if light_map.chunks.contains_key(&adjacent_chunk_position) {
                        removal_queues
                            .entry(adjacent_chunk_position)
                            .or_default()
                            .push(removal_update);
                    }
                }
            } else if light.attenuate(attenuation, true) != Light::NONE {
                let light = *light;
                for block_offset in [
                    IVec3::NEG_Y,
                    IVec3::Y,
                    IVec3::X,
                    IVec3::NEG_X,
                    IVec3::Z,
                    IVec3::NEG_Z,
                ] {
                    let block_position = BlockPosition::from(removal.index) + block_offset;
                    let index = block_position.as_chunk_index();
                    let adjacent_chunk_position =
                        chunk_position + ChunkPosition::from(block_position);

                    let propagated_light =
                        light.attenuate(attenuation, block_offset == IVec3::NEG_Y);

                    if propagated_light == Light::NONE {
                        continue;
                    }

                    let update_queue = if adjacent_chunk_position != chunk_position {
                        if let Some(adj) = light_update_queues.get_mut(&adjacent_chunk_position) {
                            adj
                        } else if light_map.chunks.contains_key(&adjacent_chunk_position) {
                            light_update_queues
                                .entry(adjacent_chunk_position)
                                .or_insert_with(LightUpdateQueue::new)
                        } else {
                            continue;
                        }
                    } else {
                        let adjacent_light = light_chunk[index];
                        if adjacent_light.sunlight() >= propagated_light.sunlight()
                            && adjacent_light.artificial() >= propagated_light.artificial()
                        {
                            continue;
                        }
                        &mut update_queue
                    };

                    update_queue.propagation.push_front(LightUpdate {
                        index,
                        light: propagated_light,
                    });
                }
            }
        }

        light_map.chunks.insert(chunk_position, light_chunk);
        light_update_queues.insert(chunk_position, update_queue);
    }
}

fn propagate_light(
    world_map: Res<WorldMap>,
    blocks: Res<Blocks>,
    mut light_update_queues: ResMut<PropagtionQueues>,
    mut light_map: ResMut<LightMap>,
    mut light_texture_uploads: ResMut<LightTextureUploads>,
) {
    for chunk_position in light_update_queues
        .keys()
        .cloned()
        .collect::<Vec<ChunkPosition>>()
    {
        let mut update_queue = light_update_queues.remove(&chunk_position).unwrap();

        let Some(mut light_chunk) = light_map.chunks.remove(&chunk_position) else {
            continue;
        };

        let Some(chunk) = world_map.get_chunk(&chunk_position) else {
            // XXX: Notice it also drops the light chunk
            continue;
        };

        // The initial sunlight propagation
        if update_queue.propagation.len() > 0 && update_queue.sunlight_pass {
            // un-attenuated sunlight is spread differently. Normal spreading is somewhat
            // expensive; and there's a lot of sunlight. A "ray" is spread all the way down,
            // passing through attenuating blocks until the light runs out. Two heights are
            // stored per column: where the ray was first attenuated and where it went dark.
            // Adjacent values in the height maps can be used to determine if there's a need to
            // spread the sunlight sideways. This way we save most of the redundant sideways
            // propagations, and spreading to adjacent chunks can be batched.
            let mut height_map: [usize; Chunk::SIZE.pow(2)] = [Chunk::SIZE; Chunk::SIZE.pow(2)];
            let mut dark_map: [usize; Chunk::SIZE.pow(2)] = [Chunk::SIZE; Chunk::SIZE.pow(2)];

            light_chunk.convert_to_normal();

            while let Some(propagation) = update_queue.propagation.pop_back() {
                if propagation.light.sunlight() != 15 {
                    update_queue.propagation.push_back(propagation);

                    break;
                }

                // Propagate the sunlight downward
                let max_y = propagation.index & 0b1111;
                let mut sunlight = propagation.light.sunlight();
                let mut attenuated = Chunk::SIZE;
                let mut index = propagation.index;
                for y in 0..=max_y {
                    index = propagation.index - y;

                    light_chunk[index].set_sunlight(sunlight);

                    let attenuation = blocks.get_config(chunk[index]).light_attenuation();
                    if sunlight == 15 && attenuation != 0 && attenuated == Chunk::SIZE {
                        attenuated = index & 0b1111;
                    }

                    sunlight = Light::new(sunlight, 0)
                        .attenuate(attenuation, true)
                        .sunlight();
                    if sunlight == 0 {
                        break;
                    }
                }
                height_map[index >> 4] = if attenuated == Chunk::SIZE {
                    0
                } else {
                    attenuated
                };
                dark_map[index >> 4] = index & 0b1111;
            }

            for x in 0..Chunk::SIZE {
                for z in 0..Chunk::SIZE {
                    // Everything at and above the attenuation height is full sunlight and can't
                    // be improved by sideways propagation.
                    let stop = height_map[x << 4 | z];

                    for (shift_x, shift_z) in [(1, 0), (0, 1), (-1, 0), (0, -1)] {
                        let shifted_x = (x as i32 + shift_x) as usize;
                        let shifted_z = (z as i32 + shift_z) as usize;
                        // If the shifted position is outside the 0..CHUNK_SIZE range, it is
                        // outside the chunk.
                        if shifted_x >= Chunk::SIZE || shifted_z >= Chunk::SIZE {
                            // We go from i32 to usize so negative values get underflowed
                            continue;
                        }

                        let from = dark_map[shifted_x << 4 | shifted_z];

                        for y in (from..stop).rev() {
                            let light = light_chunk[[shifted_x, y, shifted_z]];

                            let attenuation = blocks
                                .get_config(chunk[[shifted_x, y, shifted_z]])
                                .light_attenuation();

                            let propagated_light = light.attenuate(attenuation, false);
                            if propagated_light == Light::NONE {
                                break;
                            }

                            let existing_light = light_chunk[[x, y, z]];
                            if existing_light.sunlight() >= propagated_light.sunlight()
                                && existing_light.artificial() >= propagated_light.artificial()
                            {
                                continue;
                            }

                            update_queue.propagation.push_back(LightUpdate {
                                index: x << 8 | z << 4 | y,
                                light: propagated_light,
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

            if !changed {
                continue;
            }

            let light = *light;

            let attenuation = blocks
                .get_config(chunk[propagation.index])
                .light_attenuation();

            if light.attenuate(attenuation, true) == Light::NONE {
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
                let block_position = BlockPosition::from(propagation.index) + block_offset;
                let index = block_position.as_chunk_index();
                let adjacent_chunk_position = chunk_position + ChunkPosition::from(block_position);

                let propagated_light = light.attenuate(attenuation, block_offset == IVec3::NEG_Y);

                if propagated_light == Light::NONE {
                    continue;
                }

                let update_queue = if adjacent_chunk_position != chunk_position {
                    if update_queue.sunlight_pass {
                        // Skip, send to adjacent chunks in batch later.
                        continue;
                    }

                    if let Some(adj) = light_update_queues.get_mut(&adjacent_chunk_position) {
                        adj
                    } else if light_map.chunks.contains_key(&adjacent_chunk_position) {
                        light_update_queues
                            .entry(adjacent_chunk_position)
                            .or_insert_with(LightUpdateQueue::new)
                    } else {
                        continue;
                    }
                } else {
                    let adjacent_light = light_chunk[index];
                    if adjacent_light.sunlight() >= propagated_light.sunlight()
                        && adjacent_light.artificial() >= propagated_light.artificial()
                    {
                        continue;
                    }
                    &mut update_queue
                };

                update_queue.propagation.push_front(LightUpdate {
                    index,
                    light: propagated_light,
                });
            }
        }

        if !light_chunk.is_sunlit && update_queue.sunlight_pass {
            light_chunk.is_sunlit = true;
            update_queue.sunlight_pass = false;
            light_map.chunks.insert(chunk_position, light_chunk);
            light_map.push_to_adjacent(chunk_position, &mut light_update_queues, chunk, &blocks);
            light_map.pull_from_adjacent(chunk_position, &mut update_queue, &world_map, &blocks);
        } else {
            light_map.chunks.insert(chunk_position, light_chunk);
        }
        light_update_queues.insert(chunk_position, update_queue);
    }

    light_update_queues.retain(|chunk_position, queue| {
        if queue.propagation.is_empty() {
            if let Some(light_chunk) = light_map.chunks.get(chunk_position) {
                light_texture_uploads.insert(*chunk_position, light_chunk.as_upload());
            }
            false
        } else {
            true
        }
    });
}

fn reupload_light_texture(
    origin: Res<Origin>,
    settings: Res<Settings>,
    light_map: Res<LightMap>,
    mut light_texture_uploads: ResMut<LightTextureUploads>,
) {
    let max_distance = settings.render_distance as i32 * Chunk::SIZE as i32;
    for (chunk_position, light_chunk) in light_map.chunks.iter() {
        if (*chunk_position - origin.0).abs().max_element() > max_distance {
            continue;
        }
        light_texture_uploads.insert(*chunk_position, light_chunk.as_upload());
    }
}

fn light_chunk_unloading(world_map: Res<WorldMap>, mut light_map: ResMut<LightMap>) {
    light_map
        .chunks
        .retain(|position, _| world_map.contains_chunk(position));
}
