use bevy::{
    math::DVec3,
    tasks::{futures_lite::future, AsyncComputeTaskPool, Task},
    utils::{HashMap, HashSet},
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
        chunk::{Chunk, ChunkFace},
        RenderDistance, WorldMap,
    },
};

use super::chunk::ChunkPosition;

// Handles loading/unloading, generation and sending chunks to the players.
pub struct ChunkManagerPlugin;
impl Plugin for ChunkManagerPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<ChunkUnloadEvent>()
            .add_event::<ChunkLoadEvent>()
            .add_event::<ChunkSubscriptionEvent>()
            .add_event::<ChunkSimulationEvent>()
            .insert_resource(ChunkSubscriptions::default())
            .insert_resource(SimulatedChunks::default())
            .add_systems(
                Update,
                (
                    add_player_chunk_tracker,
                    update_player_chunk_origin,
                    update_simulated_chunks.after(update_player_chunk_origin),
                    (
                        subscribe_to_visible_chunks,
                        manage_subscriptions,
                        handle_chunk_loading_tasks,
                    )
                        .chain()
                        .after(update_player_chunk_origin),
                    unload_chunks,
                ),
            );
    }
}

#[derive(Component)]
struct ChunkTracker {
    // The chunk the player is currently/previously in. Ticks they differ are when the player
    // changed, otherwise they are equal.
    current_origin: ChunkPosition,
    prev_origin: ChunkPosition,
    // Set whenever a new chunk is loaded so it can attempt to load more
    try_subscribe: bool,
}

impl ChunkTracker {
    fn new(chunk_position: ChunkPosition) -> Self {
        Self {
            current_origin: chunk_position,
            prev_origin: chunk_position,
            try_subscribe: true,
        }
    }

    fn set_origin(&mut self, chunk_position: ChunkPosition) {
        self.prev_origin = self.current_origin;
        self.current_origin = chunk_position;
    }
}

fn add_player_chunk_tracker(
    mut commands: Commands,
    player_query: Query<(Entity, &GlobalTransform), Added<Player>>,
) {
    for (entity, transform) in player_query.iter() {
        commands
            .entity(entity)
            .insert(ChunkTracker::new(ChunkPosition::from(
                transform.translation(),
            )));
    }
}

fn update_player_chunk_origin(mut player_query: Query<(&mut ChunkTracker, &GlobalTransform)>) {
    for (mut chunk_tracker, transform) in player_query.iter_mut() {
        let chunk_position = ChunkPosition::from(transform.translation());
        chunk_tracker.set_origin(chunk_position);
    }
}

/// Sent when a player subscribes to a new chunk
#[derive(Event)]
pub struct ChunkSubscriptionEvent {
    pub player_entity: Entity,
    pub chunk_position: ChunkPosition,
}

/// Event sent when a chunk is being unloaded.
#[derive(Event, Debug)]
pub struct ChunkUnloadEvent {
    pub position: ChunkPosition,
}

/// Event sent when a chunk has just been loaded.
#[derive(Event, Debug)]
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
    player_origin_query: Query<(Entity, &ChunkTracker, &RenderDistance), Changed<ChunkTracker>>,
    mut network_events: EventReader<NetworkEvent>,
    mut subscription_events: EventReader<ChunkSubscriptionEvent>,
    mut unload_chunk_events: EventWriter<ChunkUnloadEvent>,
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
            chunk_subscriptions
                .chunk_to_subscribers
                .insert(event.chunk_position, HashSet::from([event.player_entity]));

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

    for (entity, tracker, render_distance) in player_origin_query.iter() {
        let subscribed_chunks = chunk_subscriptions
            .subscriber_to_chunks
            .get_mut(&entity)
            .unwrap();

        let removed = subscribed_chunks.extract_if(|chunk_position| {
            let distance = (*chunk_position - tracker.current_origin).abs() / Chunk::SIZE as i32;
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
                    unload_chunk_events.send(ChunkUnloadEvent {
                        position: chunk_position,
                    });
                }
            }
        }
    }

    for event in network_events.read() {
        match event {
            NetworkEvent::Connected { entity } => {
                chunk_subscriptions
                    .subscriber_to_chunks
                    .insert(*entity, HashSet::default());
            }
            NetworkEvent::Disconnected { entity } => {
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
                            unload_chunk_events.send(ChunkUnloadEvent {
                                position: chunk_position,
                            });
                        }
                    }
                }
            }
        }
    }
}

#[derive(Component)]
struct ChunkLoadingTask(Task<(ChunkPosition, Chunk)>);

// TODO: This is too expensive to accommodate many players. I'm thinking chunks can be sorted into
// columns. If it is a chunk that contains blocks, it would be considered a column base. All chunks
// of air in succession above a base would be part of the column. More generally perhaps, all
// chunks that share the same set of visible chunk faces. You will then have a set of arbitrary
// length columns with gaps that you need to traverse. Not obvious to me how to do that, but it
// gives the advantage of traversing the chunks at the world surface(which is the most expensive
// case) column by column, almost converting it from a search in 3d to a search in 2d.
// It might also make sense to split both the column representation and the visible faces part of
// the chunk into its own struct. It is pretty how it is, but I foresee that there will be
// contention for the WorldMap. The locations that borrow it mutably will need all the time they
// can get, and this system will hog it.
//
// TODO: Optimization idea: Instead of using events, use an mpsc. Removes the only need for
// mutability, and so the players can be handled in parallel. Con: Lots of allocation? Keep queues
// for each player. Maybe the search can be done by recursion? How is stack memory even handled
// when it is done in parallel.
//
// Search for chunks by fanning out from the player's chunk position to find chunks that are
// visible to it.
fn subscribe_to_visible_chunks(
    world_map: Res<WorldMap>,
    chunk_subscriptions: Res<ChunkSubscriptions>,
    mut changed_origin_query: Query<(Entity, &mut ChunkTracker, &RenderDistance)>,
    mut subscription_events: EventWriter<ChunkSubscriptionEvent>,
    mut queue: Local<Vec<(ChunkPosition, ChunkFace, ChunkFace)>>,
    mut already_visited: Local<HashSet<ChunkPosition>>,
) {
    for (player_entity, mut chunk_tracker, render_distance) in changed_origin_query.iter_mut() {
        if !chunk_tracker.try_subscribe && chunk_tracker.current_origin == chunk_tracker.prev_origin
        {
            continue;
        }
        chunk_tracker.try_subscribe = false;

        already_visited.clear();
        already_visited.insert(chunk_tracker.current_origin);

        let subscribed_chunks = chunk_subscriptions
            .subscriber_to_chunks
            .get(&player_entity)
            .unwrap();

        if !subscribed_chunks.contains(&chunk_tracker.current_origin) {
            subscription_events.send(ChunkSubscriptionEvent {
                player_entity,
                chunk_position: chunk_tracker.current_origin,
            });
        }

        for chunk_face in [
            ChunkFace::Top,
            ChunkFace::Bottom,
            ChunkFace::Right,
            ChunkFace::Left,
            ChunkFace::Front,
            ChunkFace::Back,
        ] {
            queue.push((
                chunk_face.shift_position(chunk_tracker.current_origin),
                chunk_face.opposite(),
                chunk_face.opposite(),
            ));
        }

        // chunk_position = chunk to check
        // from_face = The chunk face the chunk was entered through.
        // main_face = The chunk face entered through at the start of the search.
        while let Some((chunk_position, from_face, main_face)) = queue.pop() {
            if !subscribed_chunks.contains(&chunk_position) {
                subscription_events.send(ChunkSubscriptionEvent {
                    player_entity,
                    chunk_position,
                });
            }

            let chunk = match world_map.get_chunk(&chunk_position) {
                Some(chunk) => chunk,
                None => {
                    continue;
                }
            };

            let surrounding = [
                ChunkFace::Front,
                ChunkFace::Back,
                ChunkFace::Left,
                ChunkFace::Right,
                ChunkFace::Top,
                ChunkFace::Bottom,
            ]
            .into_iter()
            .filter(|face| *face != main_face && *face != from_face);

            for to_face in surrounding {
                let adjacent_position = to_face.shift_position(chunk_position);
                let distance_to_adjacent =
                    *(adjacent_position - chunk_tracker.current_origin) / Chunk::SIZE as i32;
                if distance_to_adjacent
                    .abs()
                    .cmpgt(IVec3::splat(render_distance.chunks as i32))
                    .any()
                {
                    continue;
                }

                if chunk.is_neighbour_visible(from_face, to_face) {
                    if !already_visited.insert(adjacent_position) {
                        // insert returns false if the position is in the set
                        continue;
                    }

                    queue.push((
                        to_face.shift_position(chunk_position),
                        to_face.opposite(),
                        main_face,
                    ));
                }
            }
        }
    }
}

fn handle_chunk_loading_tasks(
    mut commands: Commands,
    net: Res<Server>,
    mut world_map: ResMut<WorldMap>,
    chunk_subscriptions: Res<ChunkSubscriptions>,
    mut chunk_tracker_query: Query<&mut ChunkTracker>,
    mut chunks: Query<(Entity, &mut ChunkLoadingTask)>,
    mut chunk_load_event_writer: EventWriter<ChunkLoadEvent>,
) {
    for (entity, mut task) in chunks.iter_mut() {
        if let Some((new_chunk_position, chunk)) = future::block_on(future::poll_once(&mut task.0))
        {
            commands.entity(entity).despawn();

            chunk_load_event_writer.send(ChunkLoadEvent {
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
                                if let Some(mut rotation_transform) =
                                    block_config.placement.rotation_transform
                                {
                                    rotation_transform
                                        .rotate_around(DVec3::ZERO, rotation.as_quat());
                                    transform.translation += rotation_transform.translation;
                                    transform.rotation *= rotation_transform.rotation;
                                    transform.scale *= rotation_transform.scale;
                                }
                            }
                        }

                        entity_commands.insert((Model::Asset(model_id), transform));
                    }

                    chunk.block_entities.insert(index, entity_commands.id());
                }
            }

            // Triggers 'subscribe_to_visible_chunks' to run again so it can continue from
            // where it last stopped.
            let mut iter = chunk_tracker_query.iter_many_mut(subscribers.iter());
            while let Some(mut tracker) = iter.fetch_next() {
                tracker.try_subscribe = true;
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
    mut unload_chunk_events: EventReader<ChunkUnloadEvent>,
) {
    for event in unload_chunk_events.read() {
        let Some(chunk) = world_map.remove_chunk(&event.position) else {
            // A chunk might be unsubscribed to in the space of time it takes to generate it, so it
            // will never be added to the world map.
            continue;
        };

        for entity in chunk.block_entities.values() {
            commands.entity(*entity).despawn_recursive();
        }
    }
}

#[derive(Resource, Default)]
struct SimulatedChunks {
    chunks: HashMap<ChunkPosition, u32>,
}

impl SimulatedChunks {
    fn insert(&mut self, chunk_position: ChunkPosition) -> bool {
        if let Some(count) = self.chunks.get_mut(&chunk_position) {
            *count += 1;
            false
        } else {
            self.chunks.insert(chunk_position, 1);
            true
        }
    }

    fn remove(&mut self, chunk_position: &ChunkPosition) -> bool {
        let count = self.chunks.get_mut(chunk_position).unwrap();
        *count -= 1;
        if *count == 0 {
            self.chunks.remove(chunk_position);
            true
        } else {
            false
        }
    }

    fn is_simulated(&self, chunk_position: &ChunkPosition) -> bool {
        self.chunks.contains_key(chunk_position)
    }
}

#[derive(Event)]
pub enum ChunkSimulationEvent {
    Start(ChunkPosition),
    Stop(ChunkPosition),
}

const SIMULATION_DISTANCE: i32 = 5;

fn update_simulated_chunks(
    world_map: Res<WorldMap>,
    mut simulated_chunks: ResMut<SimulatedChunks>,
    players: Query<Ref<ChunkTracker>>,
    mut chunk_load_events: EventReader<ChunkLoadEvent>,
    mut chunk_unload_events: EventReader<ChunkUnloadEvent>,
    mut simulation_event_writer: EventWriter<ChunkSimulationEvent>,
) {
    for chunk_tracker in players.iter() {
        if chunk_tracker.is_added() {
            for x in -SIMULATION_DISTANCE..=SIMULATION_DISTANCE {
                for y in -SIMULATION_DISTANCE..=SIMULATION_DISTANCE {
                    for z in -SIMULATION_DISTANCE..=SIMULATION_DISTANCE {
                        let x = x * Chunk::SIZE as i32;
                        let y = y * Chunk::SIZE as i32;
                        let z = z * Chunk::SIZE as i32;
                        let chunk_position =
                            ChunkPosition::new(x, y, z) + chunk_tracker.current_origin;

                        if simulated_chunks.insert(chunk_position)
                            && world_map.contains_chunk(&chunk_position)
                        {
                            simulation_event_writer
                                .send(ChunkSimulationEvent::Start(chunk_position));
                        }
                    }
                }
            }
        } else if chunk_tracker.current_origin != chunk_tracker.prev_origin {
            for x in -SIMULATION_DISTANCE..=SIMULATION_DISTANCE {
                for z in -SIMULATION_DISTANCE..=SIMULATION_DISTANCE {
                    for y in -SIMULATION_DISTANCE..=SIMULATION_DISTANCE {
                        let x = x * Chunk::SIZE as i32;
                        let y = y * Chunk::SIZE as i32;
                        let z = z * Chunk::SIZE as i32;
                        let offset = ChunkPosition::new(x, y, z);
                        let old_position = chunk_tracker.prev_origin + offset;
                        let new_position = chunk_tracker.current_origin + offset;

                        if (new_position - chunk_tracker.prev_origin)
                            .abs()
                            .max_element()
                            > SIMULATION_DISTANCE * Chunk::SIZE as i32
                        {
                            if simulated_chunks.insert(new_position)
                                && world_map.contains_chunk(&new_position)
                            {
                                simulation_event_writer
                                    .send(ChunkSimulationEvent::Start(new_position));
                            }
                        }

                        if (old_position - chunk_tracker.current_origin)
                            .abs()
                            .max_element()
                            > SIMULATION_DISTANCE * Chunk::SIZE as i32
                        {
                            if simulated_chunks.remove(&old_position)
                                && world_map.contains_chunk(&old_position)
                            {
                                simulation_event_writer
                                    .send(ChunkSimulationEvent::Stop(old_position));
                            }
                        }
                    }
                }
            }
        }
    }

    for chunk_load_event in chunk_load_events.read() {
        if simulated_chunks.is_simulated(&chunk_load_event.position) {
            simulation_event_writer.send(ChunkSimulationEvent::Start(chunk_load_event.position));
        }
    }

    for chunk_unload_event in chunk_unload_events.read() {
        if simulated_chunks.is_simulated(&chunk_unload_event.position) {
            simulation_event_writer.send(ChunkSimulationEvent::Stop(chunk_unload_event.position));
        }
    }
}
