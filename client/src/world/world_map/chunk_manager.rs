use bevy::prelude::*;

use std::collections::HashSet;

use fmc_protocol::messages;

use crate::{
    game_state::GameState,
    networking::NetworkClient,
    rendering::RenderSet,
    settings::Settings,
    world::{
        blocks::{Block, BlockState, Blocks},
        world_map::{
            chunk::{Chunk, ChunkFace, ChunkMarker},
            WorldMap,
        },
        MovesWithOrigin, Origin,
    },
};

/// Keeps track of which chunks should be loaded/unloaded.
pub struct ChunkManagerPlugin;
impl Plugin for ChunkManagerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Pause>()
            .add_event::<NewChunkEvent>()
            .add_systems(
                Update,
                (
                    handle_new_chunks,
                    prepare_for_frustum_culling,
                    handle_block_updates
                        .after(handle_new_chunks)
                        .in_set(RenderSet::UpdateBlocks),
                    pause_system,
                )
                    .run_if(in_state(GameState::Playing)),
            )
            .add_systems(
                // BUG: Other systems add components to the chunk entity. If we're unlucky this
                // system's commands will despawn the entity before those commands are applied.
                //
                // Ambiguous system order
                // A <-> B
                // B -> A
                // Command application order
                //
                // Insert a component on an entity in A, despawn the entity in B
                // result:
                // Apply B: despawn, Apply A: panic, no entity
                //
                // Relevant issues:
                //  https://github.com/bevyengine/bevy/issues/10122
                //  https://github.com/bevyengine/bevy/issues/3845
                //
                // To work around this, it is run in PostUpdate. Systems that change components are
                // kept in Update.
                PostUpdate,
                unload_chunks.run_if(resource_changed::<Origin>),
            );
    }
}

/// Event sent after a chunk has been added to the world map
#[derive(Event)]
pub struct NewChunkEvent {
    pub position: IVec3,
}

#[derive(Resource, Default)]
struct Pause(bool);

fn pause_system(mut pause: ResMut<Pause>, keyboard_input: Res<ButtonInput<KeyCode>>) {
    if keyboard_input.just_pressed(KeyCode::F5) {
        pause.0 = !pause.0;
    }
}

// Removes chunks that are outside the render distance of the player.
fn unload_chunks(
    origin: Res<Origin>,
    mut world_map: ResMut<WorldMap>,
    settings: Res<Settings>,
    mut commands: Commands,
) {
    world_map.chunks.retain(|chunk_pos, chunk| {
        let distance = (*chunk_pos - origin.0).abs() / IVec3::splat(Chunk::SIZE as i32);
        if distance
            .cmpgt(IVec3::splat(settings.render_distance as i32) + 1)
            .any()
        {
            if let Some(entity) = chunk.entity {
                commands.entity(entity).despawn();
            }
            false
        } else {
            true
        }
    });
}

// The frustum chunk loading system needs some help. This loads the 3x3x3 chunks that are closest.
// This is for when the player walks into a chunk without looking at it first. The player might
// also collide with these without having looked at them (or collide with a chunk that isn't
// actually visible)
//fn proximity_chunk_loading(
//    origin: Res<Origin>,
//    world_map: Res<WorldMap>,
//    player_position: Query<&GlobalTransform, With<Player>>,
//    mut chunk_request_events: EventWriter<ChunkRequestEvent>,
//    pause: Res<Pause>,
//) {
//    if pause.0 {
//        return;
//    }
//    let player_position = player_position.single();
//    let player_chunk_position = utils::world_position_to_chunk_pos(
//        player_position.translation().floor().as_ivec3() + origin.0,
//    );
//
//    for x in (player_chunk_position.x - CHUNK_SIZE as i32
//        ..player_chunk_position.x + CHUNK_SIZE as i32)
//        .step_by(CHUNK_SIZE)
//    {
//        for y in (player_chunk_position.y - CHUNK_SIZE as i32
//            ..player_chunk_position.y + CHUNK_SIZE as i32)
//            .step_by(CHUNK_SIZE)
//        {
//            for z in (player_chunk_position.z - CHUNK_SIZE as i32
//                ..player_chunk_position.z + CHUNK_SIZE as i32)
//                .step_by(CHUNK_SIZE)
//            {
//                let position = IVec3::new(x, y, z);
//                if !world_map.contains_chunk(&position) {
//                    chunk_request_events.send(ChunkRequestEvent(position));
//                }
//            }
//        }
//    }
//}

// TODO: When implementing this I wanted to create chunk columns where all vertically adjacent air
// chunks belonged to the same column (with the first chunk with blocks below them as the column
// base). This would reduce the search drastically when at the surface as you could check entire
// columns in one step, instead of going through all their chunks individually. Didn't do it because
// it was too hard to imagine how it would work. Went with simpler version to save time. Maybe
// implement this or maybe ray tracing can solve it. Meanwhile, it will take up a huge chunk of the
// frame time.
// TODO: This could be made to do culling too. It's not fast enough to run each frame, but running
// it when the player looks through a different chunk face could be good enough.
//
// This traverses all chunks that are visible from the chunk the player is currently in. It does
// this by fanning out from the origin chunk, each step it takes, it marks the direction it entered
// the chunk by up to a total of three directions. From then on it can only travel in those
// directions. This makes it so that for example chunks that are on the other side of a mountain
// are marked as not visible, culling the amount of chunks that need to be rendered.
fn prepare_for_frustum_culling(
    origin: Res<Origin>,
    world_map: Res<WorldMap>,
    pause: Res<Pause>,
    mut chunk_query: Query<&mut Visibility, With<ChunkMarker>>,
    mut already_visited: Local<HashSet<IVec3>>,
    mut queue: Local<Vec<(IVec3, [ChunkFace; 3])>>,
) {
    if !origin.is_changed() {
        return;
    }

    if pause.0 {
        return;
    }

    already_visited.clear();

    // Reset the visibility of all chunks
    chunk_query.iter_mut().for_each(|mut visibility| {
        *visibility = Visibility::Hidden;
    });

    queue.push((origin.0, [ChunkFace::None; 3]));

    while let Some((chunk_position, to_faces)) = queue.pop() {
        if !already_visited.insert(chunk_position) {
            // insert returns false if the position is in the set
            continue;
        }

        let chunk = match world_map.get_chunk(&chunk_position) {
            Some(chunk) => chunk,
            None => {
                continue;
            }
        };

        if let Some(entity) = chunk.entity {
            if let Ok(mut visibility) = chunk_query.get_mut(entity) {
                *visibility = Visibility::Visible;
            }
        }

        if to_faces[0] == ChunkFace::None {
            for chunk_face in [
                ChunkFace::Top,
                ChunkFace::Bottom,
                ChunkFace::Right,
                ChunkFace::Left,
                ChunkFace::Front,
                ChunkFace::Back,
            ] {
                queue.push((
                    chunk_face.shift_position(chunk_position),
                    [chunk_face, ChunkFace::None, ChunkFace::None],
                ));
            }
            continue;
        } else {
            queue.push((to_faces[0].shift_position(chunk_position), to_faces));
        }

        if to_faces[1] == ChunkFace::None {
            let surrounding = [
                ChunkFace::Front,
                ChunkFace::Back,
                ChunkFace::Left,
                ChunkFace::Right,
                ChunkFace::Top,
                ChunkFace::Bottom,
            ]
            .into_iter()
            .filter(|face| *face != to_faces[0].opposite() && *face != to_faces[0]);

            for face in surrounding {
                queue.push((
                    face.shift_position(chunk_position),
                    [to_faces[0], face, ChunkFace::None],
                ));
            }

            continue;
        } else {
            queue.push((to_faces[1].shift_position(chunk_position), to_faces));
        }

        if to_faces[2] == ChunkFace::None {
            let remaining = match to_faces[0] {
                ChunkFace::Top | ChunkFace::Bottom => match to_faces[1] {
                    ChunkFace::Right | ChunkFace::Left => [ChunkFace::Front, ChunkFace::Back],
                    ChunkFace::Front | ChunkFace::Back => [ChunkFace::Right, ChunkFace::Left],
                    _ => unreachable!(),
                },
                ChunkFace::Right | ChunkFace::Left => match to_faces[1] {
                    ChunkFace::Top | ChunkFace::Bottom => [ChunkFace::Front, ChunkFace::Back],
                    ChunkFace::Front | ChunkFace::Back => [ChunkFace::Top, ChunkFace::Bottom],
                    _ => unreachable!(),
                },
                ChunkFace::Front | ChunkFace::Back => match to_faces[1] {
                    ChunkFace::Top | ChunkFace::Bottom => [ChunkFace::Right, ChunkFace::Left],
                    ChunkFace::Right | ChunkFace::Left => [ChunkFace::Top, ChunkFace::Bottom],
                    _ => unreachable!(),
                },
                ChunkFace::None => unreachable!(),
            };

            for face in remaining {
                queue.push((
                    face.shift_position(chunk_position),
                    [to_faces[0], to_faces[1], face],
                ));
            }
        } else {
            queue.push((to_faces[2].shift_position(chunk_position), to_faces))
        }
    }
}

// TODO: This could take ResMut<Events<ChunkResponse>> and drain the chunks to avoid
// reallocation. The lighting system listens for the same event, and it is nice to have the systems
// self-contained. Maybe the world map should contain only the chunk entity. This way there would
// no longer be a need for ComputeVisibleChunkFacesEvent either. Everything just listens for
// Changed<Chunk>. Accessing the world_map isn't actually a bottleneck I think, and doing a double
// lookup can't be that bad.
//
/// Handles chunks sent from the server.
fn handle_new_chunks(
    mut commands: Commands,
    net: Res<NetworkClient>,
    origin: Res<Origin>,
    mut world_map: ResMut<WorldMap>,
    mut new_chunk_events: EventWriter<NewChunkEvent>,
    mut received_chunks: EventReader<messages::Chunk>,
) {
    for chunk in received_chunks.read() {
        let blocks = Blocks::get();

        // TODO: Need to validate block state too. Server can crash client.
        for block_id in chunk.blocks.iter() {
            if !blocks.contains(*block_id) {
                net.disconnect(format!(
                    "Server sent chunk with unknown block id: '{}'",
                    block_id
                ));
                return;
            }
        }

        new_chunk_events.send(NewChunkEvent {
            position: chunk.position,
        });

        // TODO: Only handles uniform air chunks. These ifs can be collapsed, handle uniformity
        // in Chunk::new, skip entity like now if the chunk won't have a mesh.
        if chunk.blocks.len() == 1
            && match blocks.get_config(chunk.blocks[0]) {
                Block::Cube(b) if b.quads.len() == 0 => true,
                _ => false,
            }
        {
            world_map.insert(
                chunk.position,
                Chunk::new_air(
                    chunk.blocks.clone(),
                    chunk
                        .block_state
                        .iter()
                        .map(|(&k, &v)| (k, BlockState(v)))
                        .collect(),
                ),
            );
        } else {
            let entity = commands
                .spawn((
                    Transform::from_translation((chunk.position - origin.0).as_vec3()),
                    Visibility::Visible,
                    MovesWithOrigin,
                    ChunkMarker,
                ))
                .id();

            world_map.insert(
                chunk.position,
                Chunk::new(
                    entity,
                    chunk.blocks.clone(),
                    chunk
                        .block_state
                        .iter()
                        .map(|(&k, &v)| (k, BlockState(v)))
                        .collect(),
                ),
            );
        }
    }
}

// TODO: This doesn't feel like it belongs in this file
pub fn handle_block_updates(
    mut commands: Commands,
    net: Res<NetworkClient>,
    origin: Res<Origin>,
    mut world_map: ResMut<WorldMap>,
    mut block_updates_events: EventReader<messages::BlockUpdates>,
) {
    for event in block_updates_events.read() {
        if event.blocks.len() == 0 {
            // This is for hygiene only, although technically bad.
            net.disconnect(&format!(
                "Server error: Received an empty set of block updates."
            ));
            return;
        }

        let chunk = if let Some(c) = world_map.get_chunk_mut(&event.chunk_position) {
            c
        } else {
            // TODO: I'm not sure that this is valid. It sometimes hits this where I'd presume it would
            // not. Leaning towards there being some kind of error. It's supposed to use that tcp
            // is ordered, and chunk should be sent before any updates.
            continue;
        };

        if chunk.is_uniform() {
            chunk.convert_uniform_to_full();
            let entity = commands
                .spawn((
                    Transform::from_translation((event.chunk_position - origin.0).as_vec3()),
                    Visibility::Visible,
                    MovesWithOrigin,
                    ChunkMarker,
                ))
                .id();
            chunk.entity = Some(entity);
        }

        let blocks = Blocks::get();
        for (index, block, block_state) in event.blocks.iter() {
            if !blocks.contains(*block) {
                net.disconnect(
                    "Server sent block update with non-existing block, no block \
                    with the id: '{block}'",
                );
                return;
            }

            chunk[*index] = *block;

            if let Some(state) = block_state {
                chunk.set_block_state(*index, BlockState(*state));
            } else {
                chunk.remove_block_state(index);
            }
        }
    }
}
