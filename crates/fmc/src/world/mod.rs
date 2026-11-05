use std::{ops::Index, time::Duration};

use bevy::{
    app::AppExit,
    math::DVec3,
    platform::collections::HashMap,
    tasks::{IoTaskPool, futures_lite::future},
    time::common_conditions::on_timer,
};
use chunk::ChunkPosition;
use fmc_protocol::messages;

use crate::{
    bevy_extensions::f64_transform::TransformSystems,
    blocks::{BlockData, BlockFace, BlockId, BlockPosition, BlockState, Blocks},
    database::Database,
    models::Model,
    networking::{NetworkMessage, Server},
    players::Player,
    prelude::*,
};

pub mod chunk;
mod chunk_manager;
mod map;
mod terrain_generation;

pub use chunk_manager::{
    ChunkLoadEvent, ChunkSubscriptionEvent, ChunkSubscriptions, ChunkUnloadEvent,
};
pub use map::WorldMap;
pub use terrain_generation::{Surface, TerrainFeature, TerrainGenerator};

pub struct WorldPlugin;
impl Plugin for WorldPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(MaxRenderDistance { chunks: 16 })
            .insert_resource(BlockUpdateCache::default())
            .init_resource::<Messages<BlockUpdate>>()
            .add_message::<ChangedBlockEvent>()
            .add_plugins(chunk_manager::ChunkManagerPlugin)
            .add_systems(
                Update,
                (
                    update_chunk_positions,
                    change_player_render_distance,
                    save_blocks_to_database
                        .run_if(on_timer(Duration::from_secs(5)).or_else(on_message::<AppExit>)),
                ),
            )
            .add_systems(
                PostUpdate,
                (handle_block_updates, ApplyDeferred)
                    // We want block models to be sent immediately as they are spawned
                    // spawn -> Update GlobalTransform -> Send Model(uses GlobalTransform)
                    .before(TransformSystems::Propagate),
            );
    }
}

fn update_chunk_positions(
    mut chunk_origins: Query<(&mut ChunkPosition, &GlobalTransform), Changed<GlobalTransform>>,
) {
    for (mut chunk_position, transform) in chunk_origins.iter_mut() {
        let current_chunk_position = ChunkPosition::from(transform.translation());
        if current_chunk_position != *chunk_position {
            *chunk_position = current_chunk_position;
        }
    }
}

#[derive(Resource)]
pub struct MaxRenderDistance {
    pub chunks: u32,
}

#[derive(Component)]
pub struct RenderDistance {
    pub chunks: u32,
}

// The player may send a render distance that is less than the max to restrict the amount of chunks
// rendered.
fn change_player_render_distance(
    net: Res<Server>,
    max_render_distance: Res<MaxRenderDistance>,
    mut player_render_distance_query: Query<&mut RenderDistance>,
    mut render_distance_events: MessageReader<NetworkMessage<messages::RenderDistance>>,
) {
    for render_distance_update in render_distance_events.read() {
        let mut render_distance = player_render_distance_query
            .get_mut(render_distance_update.player_entity)
            .unwrap();

        if render_distance_update.chunks > max_render_distance.chunks {
            if net.disconnect(render_distance_update.player_entity) {
                error!(
                    "Player tried to set their render distance to {}, but the max allowed is {}, disconnecting.",
                    render_distance_update.chunks, max_render_distance.chunks
                );
            }
        }

        render_distance.chunks = render_distance_update.chunks;
    }
}

// TODO: Move block update stuff to own module
// TODO: Convert tuples to local struct "Block" to make access pretty?
//
/// Event sent in response to an applied block update.
#[derive(Message)]
pub struct ChangedBlockEvent {
    /// The position of the block that was changed.
    pub position: BlockPosition,
    pub from: (BlockId, Option<BlockState>),
    pub to: (BlockId, Option<BlockState>),
    blocks: [Option<(BlockId, Option<BlockState>)>; 27],
}

impl ChangedBlockEvent {
    fn new(
        world_map: &WorldMap,
        position: BlockPosition,
        from: (BlockId, Option<BlockState>),
    ) -> Self {
        let mut blocks = [None; 27];

        for x in -1..=1 {
            for y in -1..=1 {
                for z in -1..=1 {
                    let offset = IVec3::new(x, y, z);
                    let neighbour = position + offset;
                    blocks[Self::offset_to_index(offset)] = world_map
                        .get_block(neighbour)
                        .map(|block_id| (block_id, world_map.get_block_state(neighbour)));
                }
            }
        }

        let to = blocks[Self::offset_to_index(IVec3::ZERO)].unwrap();

        Self {
            position,
            from,
            to,
            blocks,
        }
    }

    #[track_caller]
    fn offset_to_index(offset: IVec3) -> usize {
        assert!(
            offset.abs().max_element() <= 1,
            "Tried to index a changed block's surroundings with the out of bounds offset {}",
            offset
        );
        ((offset.x + 1) * 9 + (offset.y + 1) * 3 + (offset.z + 1)) as usize
    }
}

impl Index<IVec3> for ChangedBlockEvent {
    type Output = Option<(BlockId, Option<BlockState>)>;
    #[track_caller]
    fn index(&self, offset: IVec3) -> &Self::Output {
        &self.blocks[Self::offset_to_index(offset)]
    }
}

impl Index<BlockFace> for ChangedBlockEvent {
    type Output = Option<(BlockId, Option<BlockState>)>;
    fn index(&self, block_face: BlockFace) -> &Self::Output {
        &self[block_face.offset()]
    }
}

/// Change a block in the [WorldMap]
///
/// DO NOT listen for this. If you need to know when a block changes listen for ChangedBlockEvent
#[derive(Message)]
pub enum BlockUpdate {
    /// Change one block to another.
    Replace {
        position: BlockPosition,
        block_id: BlockId,
        block_state: Option<BlockState>,
        block_data: Option<BlockData>,
    },
    // TODO: Should probably be one each for swapping data/state/id instead of having a Swap that
    // covers two.
    /// Swap out a block, keeping its entity and block data.
    Swap {
        position: BlockPosition,
        block_id: BlockId,
        block_state: Option<BlockState>,
    },
    /// Set a block's entity data
    Data {
        position: BlockPosition,
        block_data: Option<BlockData>,
    },
}

// Applies block updates to the world and sends them to the players.
fn handle_block_updates(
    mut commands: Commands,
    net: Res<Server>,
    mut world_map: ResMut<WorldMap>,
    chunk_subsriptions: Res<chunk_manager::ChunkSubscriptions>,
    mut block_events: ResMut<Messages<BlockUpdate>>,
    mut changed_block_events: MessageWriter<ChangedBlockEvent>,
    mut block_update_cache: ResMut<BlockUpdateCache>,
    mut chunked_updates: Local<HashMap<ChunkPosition, Vec<(usize, BlockId, Option<u16>)>>>,
    mut changed_blocks: Local<Vec<(BlockPosition, (BlockId, Option<BlockState>))>>,
) {
    for event in block_events.drain() {
        match event {
            BlockUpdate::Replace {
                position,
                block_id,
                block_state,
                ..
            }
            | BlockUpdate::Swap {
                position,
                block_id,
                block_state,
            } => {
                let chunk_position = ChunkPosition::from(position);
                let block_index = position.as_chunk_index();

                let Some(chunk) = world_map.get_chunk_mut(&chunk_position) else {
                    warn!("Tried to change block in non-existent chunk");
                    continue;
                };

                let prev_block = chunk.set_block(block_index, block_id);
                let prev_block_state = chunk.set_block_state(block_index, block_state);

                if let BlockUpdate::Replace { block_data, .. } = &event {
                    if let Some(old_entity) = chunk.block_entities.remove(&block_index) {
                        commands.entity(old_entity).despawn();
                    }

                    let block_config = Blocks::get().get_config(&block_id);
                    if block_config.spawn_entity_fn.is_some() || block_config.model.is_some() {
                        let mut entity_commands = commands.spawn(position);

                        if let Some(spawn_fn) = block_config.spawn_entity_fn {
                            (spawn_fn)(&mut entity_commands, block_data.as_ref());
                        }

                        if let Some(model_id) = block_config.model {
                            let mut transform = Transform::from_translation(
                                position.as_dvec3() + DVec3::new(0.5, 0.0, 0.5),
                            );

                            if let Some(block_state) = block_state {
                                if let Some(rotation) = block_state.rotation() {
                                    transform.rotate(rotation.as_quat());

                                    if let Some(custom_transform) =
                                        block_config.placement.rotation_transform
                                    {
                                        transform = transform * custom_transform;
                                    }
                                }
                            }

                            entity_commands.insert((Model::Asset(model_id), transform));
                        }

                        chunk
                            .block_entities
                            .insert(block_index, entity_commands.id());
                    }
                }

                // TODO: This is slow, see function defintion.
                chunk.check_visible_faces();

                changed_blocks.push((position, (prev_block, prev_block_state)));

                block_update_cache
                    .updates
                    .insert(position, (block_id, block_state));

                // TODO: Need to remove entries when chunks unload
                let chunked_block_updates = chunked_updates
                    .entry(chunk_position)
                    .or_insert(Vec::default());

                chunked_block_updates.push((
                    block_index,
                    block_id,
                    block_state.map(|b| b.as_u16()),
                ));
            }
            _ => (),
        }

        match event {
            BlockUpdate::Replace {
                position,
                block_data,
                ..
            }
            | BlockUpdate::Data {
                position,
                block_data,
            } => {
                block_update_cache.block_data.insert(position, block_data);
            }
            _ => (),
        }
    }

    changed_block_events.write_batch(
        changed_blocks
            .drain(..)
            .map(|(position, from)| ChangedBlockEvent::new(&world_map, position, from)),
    );

    for (chunk_position, blocks) in chunked_updates.drain() {
        if let Some(subscribers) = chunk_subsriptions.get_subscribers(&chunk_position) {
            net.send_many(
                subscribers,
                messages::BlockUpdates {
                    chunk_position: *chunk_position,
                    blocks,
                },
            );
        }
    }
}

#[derive(Default, Resource)]
struct BlockUpdateCache {
    updates: HashMap<BlockPosition, (BlockId, Option<BlockState>)>,
    block_data: HashMap<BlockPosition, Option<BlockData>>,
}

async fn save_blocks(
    database: Database,
    block_updates: HashMap<BlockPosition, (BlockId, Option<BlockState>)>,
    block_data: HashMap<BlockPosition, Option<BlockData>>,
) {
    let mut conn = database.get_read_connection();
    let transaction = conn.transaction().unwrap();
    let mut statement = transaction
        .prepare(
            r#"
        insert or replace into
            blocks (x,y,z,block_id,block_state)
        values
            (?,?,?,?,?)
        "#,
        )
        .unwrap();

    for (position, (block_id, block_state)) in block_updates {
        statement
            .execute(rusqlite::params![
                position.x,
                position.y,
                position.z,
                block_id,
                block_state.map(|state| state.as_u16())
            ])
            .unwrap();
    }
    statement.finalize().unwrap();

    let mut statement = transaction
        .prepare(
            r#"
        update blocks
        set
            block_data = ?
        where
            x = ? and y = ? and z = ?
        "#,
        )
        .unwrap();

    for (position, block_data) in block_data {
        statement
            .execute(rusqlite::params![
                block_data.map(|data| data.0),
                position.x,
                position.y,
                position.z,
            ])
            .unwrap();
    }
    statement.finalize().unwrap();

    transaction
        .commit()
        .expect("Failed to write blocks to database.");
}

fn save_blocks_to_database(
    database: Res<Database>,
    exit_events: MessageReader<AppExit>,
    mut cache: ResMut<BlockUpdateCache>,
) {
    let block_updates = cache.updates.clone();
    let block_data = cache.block_data.clone();
    cache.updates.clear();
    cache.block_data.clear();

    if !exit_events.is_empty() {
        future::block_on(save_blocks(database.clone(), block_updates, block_data));
    } else {
        let task_pool = IoTaskPool::get();
        task_pool
            .spawn(save_blocks(database.clone(), block_updates, block_data))
            .detach();
    }
}
