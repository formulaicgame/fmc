use std::{collections::HashMap, ops::Index};

use bevy::{app::AppExit, tasks::IoTaskPool};
use fmc_networking::{messages, NetworkServer};

use crate::{
    blocks::{BlockFace, BlockId, BlockPosition, BlockState, Blocks},
    database::Database,
    prelude::*,
    utils,
};

pub mod chunk;
pub mod chunk_manager;
mod map;
mod terrain_generation;

pub use chunk_manager::{ChunkSubscriptionEvent, ChunkSubscriptions};
pub use map::WorldMap;
pub use terrain_generation::{blueprints, TerrainFeature, TerrainGenerator};

pub struct WorldPlugin;

impl Plugin for WorldPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(DatabaseSyncTimer(Timer::from_seconds(
            5.0,
            TimerMode::Repeating,
        )))
        .insert_resource(RenderDistance::default())
        .add_plugins(chunk_manager::ChunkManagerPlugin)
        .add_event::<BlockUpdate>()
        .add_event::<ChangedBlockEvent>()
        .add_systems(
            PostUpdate,
            (
                handle_block_updates.run_if(on_event::<BlockUpdate>()),
                send_changed_block_event.after(handle_block_updates),
                save_block_updates_to_database,
            ),
        );
    }
}

/// Sets the preferred render distance of a player
#[derive(Resource, Component)]
pub struct RenderDistance {
    pub chunks: u32,
}

impl Default for RenderDistance {
    fn default() -> Self {
        Self { chunks: 16 }
    }
}

// TODO: Move block update stuff to own module
// TODO: Convert tuples to local struct "Block" to make access pretty?
// TODO: It might be better to remove the back_* front_* blocks. They are only used for water at
// time of writing. Adds 66% lookup time.
//
// Some types of block need to know whenever a block adjacent to it changes (for example water
// needs to know when it should spread). Instead of sending out the position of the changed block,
// this struct is constructed to save on lookup time as each system that reacts to it would need
// to query all the adjacent blocks individually.
//
/// Event sent in response to a block update.
#[derive(Event)]
pub struct ChangedBlockEvent {
    pub position: IVec3,
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

#[derive(Event)]
pub enum BlockUpdate {
    /// Change one block to another. Fields are position/block id/block state
    Change {
        position: IVec3,
        block_id: BlockId,
        block_state: Option<BlockState>,
    },
    // Particles?
}

// Applies block updates to the world and sends them to the players.
fn handle_block_updates(
    mut commands: Commands,
    net: Res<NetworkServer>,
    chunk_subsriptions: Res<chunk_manager::ChunkSubscriptions>,
    mut world_map: ResMut<WorldMap>,
    mut block_events: EventReader<BlockUpdate>,
    mut chunked_updates: Local<HashMap<IVec3, Vec<(usize, BlockId, Option<u16>)>>>,
) {
    for event in block_events.read() {
        match event {
            BlockUpdate::Change {
                position,
                block_id,
                block_state,
            } => {
                let (chunk_pos, block_index) =
                    utils::world_position_to_chunk_position_and_block_index(*position);

                let chunk = if let Some(c) = world_map.get_chunk_mut(&chunk_pos) {
                    c
                } else {
                    panic!("Tried to change block in non-existing chunk");
                };

                chunk[block_index] = *block_id;
                chunk.set_block_state(block_index, *block_state);

                if let Some(old_entity) = chunk.block_entities.remove(&block_index) {
                    commands.entity(old_entity).despawn_recursive();
                }

                let block_config = Blocks::get().get_config(block_id);
                if let Some(spawn_fn) = block_config.spawn_entity_fn {
                    let mut entity_commands = commands.spawn(BlockPosition(*position));

                    (spawn_fn)(&mut entity_commands, None);

                    chunk
                        .block_entities
                        .insert(block_index, entity_commands.id());
                }

                // XXX: This is slow, see function defintion. Put here to cause problems.
                chunk.check_visible_faces();

                let chunked_block_updates =
                    chunked_updates.entry(chunk_pos).or_insert(Vec::default());

                chunked_block_updates.push((
                    block_index,
                    *block_id,
                    block_state.map(|b| b.as_u16()),
                ));
            }
        }
    }

    for (chunk_position, blocks) in chunked_updates.drain() {
        if let Some(subscribers) = chunk_subsriptions.get_subscribers(&chunk_position) {
            net.send_many(
                subscribers,
                messages::BlockUpdates {
                    chunk_position,
                    blocks,
                },
            );
        }
    }
}

#[derive(Resource, DerefMut, Deref)]
struct DatabaseSyncTimer(Timer);

async fn save_blocks(
    database: Database,
    block_updates: Vec<(IVec3, (BlockId, Option<BlockState>))>,
) {
    let mut conn = database.get_connection();
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
                block_state.map(|state| state.0)
            ])
            .unwrap();
    }
    statement.finalize().unwrap();
    transaction
        .commit()
        .expect("Failed to write blocks to database.");
}

fn save_block_updates_to_database(
    database: Res<Database>,
    time: Res<Time>,
    mut block_events: EventReader<BlockUpdate>,
    mut sync_timer: ResMut<DatabaseSyncTimer>,
    exit_events: EventReader<AppExit>,
    mut block_updates: Local<HashMap<IVec3, (BlockId, Option<BlockState>)>>,
) {
    for event in block_events.read() {
        match event {
            BlockUpdate::Change {
                position,
                block_id,
                block_state,
            } => {
                block_updates.insert(*position, (*block_id, *block_state));
            }
        }
    }

    sync_timer.tick(time.delta());
    if sync_timer.just_finished() {
        let task_pool = IoTaskPool::get();
        let block_updates = block_updates.drain().collect();
        task_pool
            .spawn(save_blocks(database.clone(), block_updates))
            .detach();
    }

    if !exit_events.is_empty() {
        let block_updates = block_updates.drain().collect();
        futures_lite::future::block_on(save_blocks(database.clone(), block_updates));
    }
}

fn send_changed_block_event(
    world_map: Res<WorldMap>,
    mut block_update_events: EventReader<BlockUpdate>,
    mut changed_block_events: EventWriter<ChangedBlockEvent>,
) {
    changed_block_events.send_batch(block_update_events.read().map(|event| {
        match event {
            BlockUpdate::Change {
                position,
                block_id,
                block_state,
            } => ChangedBlockEvent {
                position: *position,
                to: (*block_id, *block_state),
                top: world_map
                    .get_block(*position + IVec3::Y)
                    .map(|block_id| ((block_id, world_map.get_block_state(*position + IVec3::Y)))),
                bottom: world_map
                    .get_block(*position - IVec3::Y)
                    .map(|block_id| (block_id, world_map.get_block_state(*position - IVec3::Y))),
                right: world_map
                    .get_block(*position + IVec3::X)
                    .map(|block_id| (block_id, world_map.get_block_state(*position + IVec3::X))),
                left: world_map
                    .get_block(*position - IVec3::X)
                    .map(|block_id| (block_id, world_map.get_block_state(*position - IVec3::X))),
                front: world_map
                    .get_block(*position + IVec3::Z)
                    .map(|block_id| (block_id, world_map.get_block_state(*position + IVec3::Z))),
                front_left: world_map
                    .get_block(*position + IVec3::Z - IVec3::X)
                    .map(|block_id| {
                        (
                            block_id,
                            world_map.get_block_state(*position + IVec3::Z - IVec3::X),
                        )
                    }),
                front_right: world_map
                    .get_block(*position + IVec3::Z + IVec3::X)
                    .map(|block_id| {
                        (
                            block_id,
                            world_map.get_block_state(*position + IVec3::Z + IVec3::X),
                        )
                    }),
                back: world_map
                    .get_block(*position - IVec3::Z)
                    .map(|block_id| (block_id, world_map.get_block_state(*position - IVec3::Z))),
                back_left: world_map
                    .get_block(*position - IVec3::Z - IVec3::X)
                    .map(|block_id| {
                        (
                            block_id,
                            world_map.get_block_state(*position - IVec3::Z - IVec3::X),
                        )
                    }),
                back_right: world_map
                    .get_block(*position - IVec3::Z + IVec3::X)
                    .map(|block_id| {
                        (
                            block_id,
                            world_map.get_block_state(*position - IVec3::Z + IVec3::X),
                        )
                    }),
            },
        }
    }));
}
