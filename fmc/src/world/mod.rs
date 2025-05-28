use std::{collections::HashMap, ops::Index, time::Duration};

use bevy::{
    app::AppExit,
    math::DVec3,
    tasks::{futures_lite::future, IoTaskPool},
    time::common_conditions::on_timer,
};
use chunk::ChunkPosition;
use fmc_protocol::messages;

use crate::{
    bevy_extensions::f64_transform::TransformSystem,
    blocks::{BlockData, BlockFace, BlockId, BlockPosition, BlockState, Blocks},
    database::Database,
    models::Model,
    networking::{NetworkMessage, Server},
    prelude::*,
};

pub mod chunk;
mod chunk_manager;
mod map;
mod terrain_generation;

pub use chunk_manager::{
    ChunkLoadEvent, ChunkSimulationEvent, ChunkSubscriptionEvent, ChunkSubscriptions,
    ChunkUnloadEvent,
};
pub use map::WorldMap;
pub use terrain_generation::{Surface, TerrainFeature, TerrainGenerator};

pub struct WorldPlugin;
impl Plugin for WorldPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(RenderDistance { chunks: 16 })
            .insert_resource(BlockUpdateCache::default())
            .init_resource::<Events<BlockUpdate>>()
            .add_event::<ChangedBlockEvent>()
            .add_plugins(chunk_manager::ChunkManagerPlugin)
            .add_systems(
                Update,
                (
                    update_chunk_origins,
                    change_player_render_distance,
                    save_blocks_to_database
                        .run_if(on_timer(Duration::from_secs(5)).or(on_event::<AppExit>)),
                ),
            )
            .add_systems(
                PostUpdate,
                (handle_block_updates, apply_deferred)
                    // We want block models to be sent immediately as they are spawned
                    // spawn -> Update GlobalTransform -> Send Model(uses GlobalTransform)
                    .before(TransformSystem::TransformPropagate),
            );
    }
}

/// Keeps track of which chunk an entity is in. Useful for tracking when the entity moves between
/// chunks.
#[derive(Component)]
pub struct ChunkOrigin {
    pub chunk_position: ChunkPosition,
}

impl Default for ChunkOrigin {
    fn default() -> Self {
        Self {
            chunk_position: ChunkPosition::new(0, 0, 0),
        }
    }
}

fn update_chunk_origins(
    mut chunk_origins: Query<(&mut ChunkOrigin, &GlobalTransform), Changed<GlobalTransform>>,
) {
    for (mut origin, transform) in chunk_origins.iter_mut() {
        let current_chunk_position = ChunkPosition::from(transform.translation());
        if current_chunk_position != origin.chunk_position {
            origin.chunk_position = current_chunk_position;
        }
    }
}

/// As a resource this is the max render distance the server allows. As a component it is the
/// render distance for a player (always <= the server's).
#[derive(Resource, Component)]
pub struct RenderDistance {
    pub chunks: u32,
}

// The player may send a render distance than is less than the max to restrict the amount of chunks
// rendered.
fn change_player_render_distance(
    net: Res<Server>,
    max_render_distance: Res<RenderDistance>,
    mut player_render_distance_query: Query<&mut RenderDistance>,
    mut render_distance_events: EventReader<NetworkMessage<messages::RenderDistance>>,
) {
    for render_distance_update in render_distance_events.read() {
        let mut render_distance = player_render_distance_query
            .get_mut(render_distance_update.player_entity)
            .unwrap();

        if render_distance.chunks > max_render_distance.chunks {
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
// TODO: It might be better to remove the back_* front_* blocks. They are only used for water at
// time of writing. Adds 66% to lookup time.
//
// Some blocks need to know when blocks adjacent to them change (for example water needs to know
// when it should spread). Instead of sending out the position of the changed block, this struct is
// constructed to save on lookup time as each system that reacts to it would need to query all the
// adjacent blocks separately.
//
/// Event sent in response to a block update.
#[derive(Event)]
pub struct ChangedBlockEvent {
    /// The position of the block that was changed.
    pub position: BlockPosition,
    pub from: (BlockId, Option<BlockState>),
    /// What block it was changed into
    pub to: (BlockId, Option<BlockState>),
    pub top: Option<(BlockId, Option<BlockState>)>,
    pub bottom: Option<(BlockId, Option<BlockState>)>,
    pub back: Option<(BlockId, Option<BlockState>)>,
    pub back_right: Option<(BlockId, Option<BlockState>)>,
    pub back_left: Option<(BlockId, Option<BlockState>)>,
    pub right: Option<(BlockId, Option<BlockState>)>,
    pub left: Option<(BlockId, Option<BlockState>)>,
    pub front: Option<(BlockId, Option<BlockState>)>,
    pub front_right: Option<(BlockId, Option<BlockState>)>,
    pub front_left: Option<(BlockId, Option<BlockState>)>,
}

impl Index<BlockFace> for ChangedBlockEvent {
    type Output = Option<(BlockId, Option<BlockState>)>;
    fn index(&self, index: BlockFace) -> &Self::Output {
        match index {
            BlockFace::Front => &self.front,
            BlockFace::Back => &self.back,
            BlockFace::Right => &self.right,
            BlockFace::Left => &self.left,
            BlockFace::Top => &self.top,
            BlockFace::Bottom => &self.bottom,
        }
    }
}

impl Index<[BlockFace; 2]> for ChangedBlockEvent {
    type Output = Option<(BlockId, Option<BlockState>)>;
    #[track_caller]
    fn index(&self, index: [BlockFace; 2]) -> &Self::Output {
        match index {
            [BlockFace::Front, BlockFace::Left] => &self.front_left,
            [BlockFace::Left, BlockFace::Front] => &self.front_left,
            [BlockFace::Front, BlockFace::Right] => &self.front_right,
            [BlockFace::Right, BlockFace::Front] => &self.front_right,
            [BlockFace::Back, BlockFace::Left] => &self.back_left,
            [BlockFace::Left, BlockFace::Back] => &self.back_left,
            [BlockFace::Back, BlockFace::Right] => &self.back_right,
            [BlockFace::Right, BlockFace::Back] => &self.back_right,
            _ => panic!("Tried to index with non-horizontal blockfaces."),
        }
    }
}

/// Change a block in the [WorldMap]
///
/// DO NOT listen for this. If you need to know when a block changes listen for ChangedBlockEvent
#[derive(Event)]
pub enum BlockUpdate {
    /// Change one block to another.
    Replace {
        position: BlockPosition,
        block_id: BlockId,
        block_state: Option<BlockState>,
        block_data: Option<BlockData>,
    },
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
    mut block_events: ResMut<Events<BlockUpdate>>,
    mut changed_block_events: EventWriter<ChangedBlockEvent>,
    mut block_update_cache: ResMut<BlockUpdateCache>,
    mut chunked_updates: Local<HashMap<ChunkPosition, Vec<(usize, BlockId, Option<u16>)>>>,
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

                let chunk = if let Some(c) = world_map.get_chunk_mut(&chunk_position) {
                    c
                } else {
                    panic!("Tried to change block in non-existent chunk");
                };

                let prev_block = chunk.set_block(block_index, block_id);
                let prev_block_state = chunk.set_block_state(block_index, block_state);

                if let BlockUpdate::Replace { block_data, .. } = &event {
                    if let Some(old_entity) = chunk.block_entities.remove(&block_index) {
                        commands.entity(old_entity).despawn_recursive();
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

                send_changed_block_event(
                    &world_map,
                    &mut changed_block_events,
                    position,
                    prev_block,
                    prev_block_state,
                    block_id,
                    block_state,
                );

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
    exit_events: EventReader<AppExit>,
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

fn send_changed_block_event(
    world_map: &WorldMap,
    changed_block_events: &mut EventWriter<ChangedBlockEvent>,
    position: BlockPosition,
    prev_block_id: BlockId,
    prev_block_state: Option<BlockState>,
    block_id: BlockId,
    block_state: Option<BlockState>,
) {
    changed_block_events.send(ChangedBlockEvent {
        position,
        from: (prev_block_id, prev_block_state),
        to: (block_id, block_state),
        top: world_map
            .get_block(position + IVec3::Y)
            .map(|block_id| ((block_id, world_map.get_block_state(position + IVec3::Y)))),
        bottom: world_map
            .get_block(position - IVec3::Y)
            .map(|block_id| (block_id, world_map.get_block_state(position - IVec3::Y))),
        right: world_map
            .get_block(position + IVec3::X)
            .map(|block_id| (block_id, world_map.get_block_state(position + IVec3::X))),
        left: world_map
            .get_block(position - IVec3::X)
            .map(|block_id| (block_id, world_map.get_block_state(position - IVec3::X))),
        front: world_map
            .get_block(position + IVec3::Z)
            .map(|block_id| (block_id, world_map.get_block_state(position + IVec3::Z))),
        front_left: world_map
            .get_block(position + IVec3::Z - IVec3::X)
            .map(|block_id| {
                (
                    block_id,
                    world_map.get_block_state(position + IVec3::Z - IVec3::X),
                )
            }),
        front_right: world_map
            .get_block(position + IVec3::Z + IVec3::X)
            .map(|block_id| {
                (
                    block_id,
                    world_map.get_block_state(position + IVec3::Z + IVec3::X),
                )
            }),
        back: world_map
            .get_block(position - IVec3::Z)
            .map(|block_id| (block_id, world_map.get_block_state(position - IVec3::Z))),
        back_left: world_map
            .get_block(position - IVec3::Z - IVec3::X)
            .map(|block_id| {
                (
                    block_id,
                    world_map.get_block_state(position - IVec3::Z - IVec3::X),
                )
            }),
        back_right: world_map
            .get_block(position - IVec3::Z + IVec3::X)
            .map(|block_id| {
                (
                    block_id,
                    world_map.get_block_state(position - IVec3::Z + IVec3::X),
                )
            }),
    });
}
