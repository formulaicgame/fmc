use bevy::platform::collections::{HashMap, HashSet};
use bevy::{
    math::DVec3,
    tasks::{AsyncComputeTaskPool, Task, futures_lite::future},
};
use fmc_protocol::messages;

use crate::{
    blocks::{BlockPosition, Blocks},
    database::Database,
    models::Model,
    networking::{NetworkEvent, Server},
    players::Player,
    prelude::*,
    world::{
        RenderDistance, WorldMap,
        chunk::{Chunk, ChunkFace},
    },
};

use super::chunk::ChunkPosition;

// Handles loading/unloading, generation and sending chunks to the players.
pub struct ChunkManagerPlugin;
impl Plugin for ChunkManagerPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<ChunkUnloadEvent>()
            .add_message::<ChunkLoadEvent>()
            .add_message::<ChunkSubscriptionEvent>()
            .insert_resource(ChunkSubscriptions::default())
            .add_systems(
                Update,
                (
                    (
                        add_and_remove_subscribers,
                        traversal_subscriber,
                        manage_subscriptions,
                        handle_chunk_loading_tasks,
                    )
                        .chain(),
                    unload_chunks,
                ),
            );
    }
}

#[derive(Component)]
struct SubscriptionTrigger;

fn add_and_remove_subscribers(
    mut commands: Commands,
    world_map: Res<WorldMap>,
    mut chunk_subscriptions: ResMut<ChunkSubscriptions>,
    connections: Query<Entity, Added<Player>>,
    mut disconnections: MessageReader<NetworkEvent>,
    mut unload_chunk_events: MessageWriter<ChunkUnloadEvent>,
) {
    for entity in connections.iter() {
        chunk_subscriptions
            .subscriber_to_chunks
            .insert(entity, HashSet::default());

        commands.entity(entity).insert(SubscriptionTrigger);
    }

    for network_event in disconnections.read() {
        let NetworkEvent::Disconnected { entity } = network_event else {
            continue;
        };

        let subscribed_chunks = chunk_subscriptions
            .subscriber_to_chunks
            .remove(entity)
            .unwrap();

        for chunk_position in subscribed_chunks {
            let subscribers = chunk_subscriptions
                .chunk_to_subscribers
                .get_mut(&chunk_position)
                .unwrap();
            subscribers.remove(entity);

            if subscribers.len() == 0 {
                chunk_subscriptions
                    .chunk_to_subscribers
                    .remove(&chunk_position);
                if world_map.contains_chunk(&chunk_position) {
                    unload_chunk_events.write(ChunkUnloadEvent {
                        position: chunk_position,
                    });
                }
            }
        }
    }
}

/// Sent when a player subscribes to a new chunk
#[derive(Message)]
pub struct ChunkSubscriptionEvent {
    pub player_entity: Entity,
    pub chunk_position: ChunkPosition,
}

/// Event sent when a chunk is being unloaded.
#[derive(Message, Debug)]
pub struct ChunkUnloadEvent {
    pub position: ChunkPosition,
}

/// Event sent when a chunk has just been loaded.
#[derive(Message, Debug)]
pub struct ChunkLoadEvent {
    /// The position of the chunk
    pub position: ChunkPosition,
}

// Keeps track of which players are subscribed to which chunks. Clients will get updates for
// everything that happens within a chunk it is subscribed to.
#[derive(Resource, Default)]
pub struct ChunkSubscriptions {
    chunk_to_subscribers: HashMap<ChunkPosition, HashSet<Entity>>,
    subscriber_to_chunks: HashMap<Entity, HashSet<ChunkPosition>>,
}

impl ChunkSubscriptions {
    pub fn get_subscribers(&self, chunk_position: &ChunkPosition) -> Option<&HashSet<Entity>> {
        return self.chunk_to_subscribers.get(chunk_position);
    }
}

fn manage_subscriptions(
    mut commands: Commands,
    net: Res<Server>,
    world_map: Res<WorldMap>,
    database: Res<Database>,
    mut chunk_subscriptions: ResMut<ChunkSubscriptions>,
    player_query: Query<
        (Entity, &ChunkPosition, &RenderDistance),
        Or<(Changed<ChunkPosition>, Changed<RenderDistance>)>,
    >,
    mut subscription_events: MessageReader<ChunkSubscriptionEvent>,
    mut unload_chunk_events: MessageWriter<ChunkUnloadEvent>,
) {
    let thread_pool = AsyncComputeTaskPool::get();

    for event in subscription_events.read() {
        chunk_subscriptions
            .subscriber_to_chunks
            .get_mut(&event.player_entity)
            .unwrap()
            .insert(event.chunk_position);

        if let Some(chunk_subscribers) = chunk_subscriptions
            .chunk_to_subscribers
            .get_mut(&event.chunk_position)
        {
            chunk_subscribers.insert(event.player_entity);
            if let Some(chunk) = world_map.get_chunk(&event.chunk_position) {
                net.send_one(
                    event.player_entity,
                    messages::Chunk {
                        position: *event.chunk_position,
                        blocks: chunk.blocks.clone(),
                        block_state: chunk.block_state.clone(),
                    },
                );
            }
        } else {
            chunk_subscriptions.chunk_to_subscribers.insert(
                event.chunk_position,
                HashSet::from_iter([event.player_entity]),
            );

            let task = thread_pool.spawn(Chunk::load(
                event.chunk_position,
                world_map.terrain_generator.clone(),
                database.clone(),
            ));

            commands.spawn(ChunkLoadingTask(task));
        };
    }

    // reborrow for split borrow
    let chunk_subscriptions = chunk_subscriptions.into_inner();

    for (entity, player_position, render_distance) in player_query.iter() {
        let subscribed_chunks = chunk_subscriptions
            .subscriber_to_chunks
            .get_mut(&entity)
            .unwrap();

        let removed = subscribed_chunks.extract_if(|chunk_position| {
            let distance = (*chunk_position - *player_position).abs() / Chunk::SIZE as i32;
            if distance
                .cmpgt(IVec3::splat(render_distance.chunks as i32))
                .any()
            {
                return true;
            } else {
                return false;
            }
        });

        for chunk_position in removed {
            let chunk_subscribers = chunk_subscriptions
                .chunk_to_subscribers
                .get_mut(&chunk_position)
                .unwrap();
            chunk_subscribers.remove(&entity);

            if chunk_subscribers.len() == 0 {
                chunk_subscriptions
                    .chunk_to_subscribers
                    .remove(&chunk_position);
                if world_map.contains_chunk(&chunk_position) {
                    unload_chunk_events.write(ChunkUnloadEvent {
                        position: chunk_position,
                    });
                }
            }
        }
    }
}

#[derive(Component)]
struct ChunkLoadingTask(Task<(ChunkPosition, Chunk)>);

// TODO: Optimization idea: The players can be handled in parallel with par_iter.
// Use Local<Parallel<(chunk_faces, chunk_visible, sub_events)>>. After the par_iter, drain each
// thread's events into the MessageWriter with write_batch. Destructure the scratch struct to get
// independent borrows of the buffers.
//
// This finds all chunks that are visible from the chunk the player is currently in.
// A chunk is visible if it can be reached from the player's chunk through the
// interior of loaded chunks by only ever moving away from the player.
fn traversal_subscriber(
    world_map: Res<WorldMap>,
    chunk_subscriptions: Res<ChunkSubscriptions>,
    player_query: Query<
        (Entity, &ChunkPosition, &RenderDistance),
        Or<(
            Changed<ChunkPosition>,
            Changed<SubscriptionTrigger>,
            Changed<RenderDistance>,
        )>,
    >,
    mut subscription_events: MessageWriter<ChunkSubscriptionEvent>,
    mut chunk_faces: Local<Vec<[u8; 6]>>,
    mut chunk_visible: Local<Vec<u8>>,
) {
    for (player_entity, player_position, render_distance) in player_query.iter() {
        let radius = render_distance.chunks as i32;
        let width = radius * 2 + 1;

        chunk_faces.clear();
        chunk_faces.resize(width.pow(3) as usize, [0; 6]);
        chunk_visible.clear();
        chunk_visible.resize(width.pow(3) as usize, 0);

        let min = **player_position - IVec3::splat(radius * Chunk::SIZE as i32);

        let grid_index = |offset: IVec3| -> usize {
            ((offset.x * width + offset.y) * width + offset.z) as usize
        };

        let center = IVec3::splat(radius);
        chunk_faces[grid_index(center)] = [0b111111; 6];
        chunk_visible[grid_index(center)] = 0b111111;

        for octant in [
            IVec3::new(1, 1, 1),
            IVec3::new(1, 1, -1),
            IVec3::new(1, -1, 1),
            IVec3::new(1, -1, -1),
            IVec3::new(-1, 1, 1),
            IVec3::new(-1, 1, -1),
            IVec3::new(-1, -1, 1),
            IVec3::new(-1, -1, -1),
        ] {
            // The face the neighbouring chunk is exited through when moving towards this chunk.
            let x_exit = if octant.x == 1 {
                ChunkFace::Right
            } else {
                ChunkFace::Left
            };
            let y_exit = if octant.y == 1 {
                ChunkFace::Top
            } else {
                ChunkFace::Bottom
            };
            let z_exit = if octant.z == 1 {
                ChunkFace::Front
            } else {
                ChunkFace::Back
            };

            for dx in 0..=radius {
                for dy in 0..=radius {
                    for dz in 0..=radius {
                        if dx == 0 && dy == 0 && dz == 0 {
                            continue;
                        }

                        let offset = center + IVec3::new(dx, dy, dz) * octant;

                        let mut visible = 0;
                        if dx > 0 {
                            let neighbour = grid_index(offset - IVec3::new(octant.x, 0, 0));
                            if chunk_visible[neighbour] & chunk_faces[neighbour][x_exit as usize]
                                != 0
                            {
                                visible |= 1 << x_exit.opposite() as usize;
                            }
                        }
                        if dy > 0 {
                            let neighbour = grid_index(offset - IVec3::new(0, octant.y, 0));
                            if chunk_visible[neighbour] & chunk_faces[neighbour][y_exit as usize]
                                != 0
                            {
                                visible |= 1 << y_exit.opposite() as usize;
                            }
                        }
                        if dz > 0 {
                            let neighbour = grid_index(offset - IVec3::new(0, 0, octant.z));
                            if chunk_visible[neighbour] & chunk_faces[neighbour][z_exit as usize]
                                != 0
                            {
                                visible |= 1 << z_exit.opposite() as usize;
                            }
                        }

                        if visible != 0 {
                            let index = grid_index(offset);
                            chunk_visible[index] = visible;

                            let chunk_position =
                                ChunkPosition::from(min + offset * Chunk::SIZE as i32);
                            if let Some(chunk) = world_map.get_chunk(&chunk_position) {
                                chunk_faces[index] = chunk.visible_faces;
                            }
                        }
                    }
                }
            }
        }

        let subscribed_chunks = chunk_subscriptions
            .subscriber_to_chunks
            .get(&player_entity)
            .unwrap();

        for (index, visible) in chunk_visible.iter().enumerate() {
            if *visible == 0 {
                continue;
            }

            let index = index as i32;
            let offset = IVec3::new(
                index / (width * width),
                index / width % width,
                index % width,
            );
            let chunk_position = ChunkPosition::from(min + offset * Chunk::SIZE as i32);

            if !subscribed_chunks.contains(&chunk_position) {
                subscription_events.write(ChunkSubscriptionEvent {
                    player_entity,
                    chunk_position,
                });
            }
        }
    }
}

fn handle_chunk_loading_tasks(
    mut commands: Commands,
    net: Res<Server>,
    mut world_map: ResMut<WorldMap>,
    chunk_subscriptions: Res<ChunkSubscriptions>,
    mut subscription_triggers: Query<&mut SubscriptionTrigger>,
    mut chunks: Query<(Entity, &mut ChunkLoadingTask)>,
    mut chunk_load_event_writer: MessageWriter<ChunkLoadEvent>,
) {
    for (entity, mut task) in chunks.iter_mut() {
        if let Some((new_chunk_position, chunk)) = future::block_on(future::poll_once(&mut task.0))
        {
            commands.entity(entity).despawn();

            chunk_load_event_writer.write(ChunkLoadEvent {
                position: new_chunk_position,
            });

            let Some(subscribers) = chunk_subscriptions
                .chunk_to_subscribers
                .get(&new_chunk_position)
            else {
                // Discard the chunk if it got unsubscribed to while loading
                continue;
            };

            world_map.insert(new_chunk_position, chunk);

            // XXX: Terrain features that fit within the chunk should be applied at generation!
            for adjacent_chunk_position in new_chunk_position.neighbourhood() {
                if adjacent_chunk_position == new_chunk_position {
                    continue;
                }

                let chunk = match world_map.get_chunk_mut(&adjacent_chunk_position) {
                    Some(c) => c,
                    // x,y,z = 0, ignored here
                    None => continue,
                };

                // Since we need mutable access to all chunks to apply terrain features we
                // need to temporarily remove them to satisfy the borrow checker.
                let terrain_features =
                    std::mem::replace(&mut chunk.terrain_features, Vec::default());

                for terrain_feature in terrain_features.iter() {
                    if !terrain_feature.applies_to_chunk(&new_chunk_position)
                        || terrain_feature.fits_in_chunk(new_chunk_position)
                    {
                        // Skip if the feature doesn't apply to the generated chunk or if
                        // it is one of the features that fit within the chunk and has thus
                        // already been placed.
                        continue;
                    }

                    for (chunk_position, blocks) in
                        terrain_feature.apply_edge_feature(&mut world_map)
                    {
                        if chunk_position == new_chunk_position {
                            // No need to send a block updates for the new chunk as it
                            // hasn't been sent yet.
                            continue;
                        }

                        if let Some(subscribers) =
                            chunk_subscriptions.get_subscribers(&chunk_position)
                        {
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

                // And we move the terrain features back
                let chunk = world_map.get_chunk_mut(&adjacent_chunk_position).unwrap();
                chunk.terrain_features = terrain_features;
            }

            let chunk = world_map.get_chunk_mut(&new_chunk_position).unwrap();

            let blocks = Blocks::get();
            for (index, block_id) in chunk.blocks.iter().enumerate() {
                let block_config = blocks.get_config(block_id);
                if block_config.spawn_entity_fn.is_some() || block_config.model.is_some() {
                    let mut entity_commands = commands.spawn_empty();

                    let block_position =
                        BlockPosition::from(new_chunk_position) + BlockPosition::from(index);
                    entity_commands.insert(block_position);

                    if let Some(function) = block_config.spawn_entity_fn {
                        let block_data = chunk.block_data.remove(&index);
                        (function)(&mut entity_commands, block_data.as_ref());

                        if let Some(block_data) = block_data {
                            entity_commands.insert(block_data);
                        }
                    }

                    if let Some(model_id) = block_config.model {
                        let mut transform = Transform::from_translation(
                            block_position.as_dvec3() + DVec3::new(0.5, 0.0, 0.5),
                        );
                        if let Some(block_state) = chunk.get_block_state(&index) {
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

                    chunk.block_entities.insert(index, entity_commands.id());
                }
            }

            // Triggers 'traversal_subscriber' to run again so it can continue from
            // where it last stopped.
            let mut iter = subscription_triggers.iter_many_mut(subscribers.iter());
            while let Some(mut trigger) = iter.fetch_next() {
                trigger.set_changed();
            }

            net.send_many(
                subscribers,
                messages::Chunk {
                    position: *new_chunk_position,
                    blocks: chunk.blocks.clone(),
                    block_state: chunk.block_state.clone(),
                },
            );
        }
    }
}

fn unload_chunks(
    mut commands: Commands,
    mut world_map: ResMut<WorldMap>,
    mut unload_chunk_events: MessageReader<ChunkUnloadEvent>,
) {
    for event in unload_chunk_events.read() {
        let Some(chunk) = world_map.remove_chunk(&event.position) else {
            // A chunk might be unsubscribed to in the space of time it takes to generate it, so it
            // will never be added to the world map.
            continue;
        };

        for entity in chunk.block_entities.values() {
            commands.entity(*entity).despawn();
        }
    }
}
