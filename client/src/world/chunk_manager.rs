use bevy::prelude::*;

use fmc_protocol::messages;

use crate::{
    blocks::{BlockState, Blocks},
    game_state::GameState,
    networking::NetworkClient,
    rendering::RenderSet,
    settings::Settings,
    world::{
        MovesWithOrigin, Origin, WorldMap,
        chunk::{Chunk, ChunkMarker},
    },
};

pub struct ChunkManagerPlugin;
impl Plugin for ChunkManagerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Pause>()
            .add_message::<NewChunkEvent>()
            .add_systems(
                Update,
                (
                    handle_new_chunks,
                    occlusion_culling,
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
                //
                // A <-> B Ambiguous system order
                // B -> A Command application order
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
                unload_chunks
                    .run_if(resource_changed::<Origin>.or_else(resource_changed::<Settings>)),
            );
    }
}

/// Event sent after a chunk has been added to the world map
#[derive(Message)]
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
    settings: Res<Settings>,
    origin: Res<Origin>,
    mut world_map: ResMut<WorldMap>,
    mut commands: Commands,
) {
    world_map.chunks.retain(|chunk_pos, chunk| {
        let distance = (*chunk_pos - origin.0).abs() / IVec3::splat(Chunk::SIZE as i32);
        if distance
            .cmpgt(IVec3::splat(settings.render_distance as i32))
            .any()
        {
            if let Some(entity) = chunk.entity() {
                commands.entity(entity).despawn();
            }
            false
        } else {
            true
        }
    });
}

// TODO: This could be made to do frustum culling too. Running it when the player looks through a
// different chunk face could be good enough so it doesn't have to be done every frame.
//
// Does some crude occlusion culling in order to get rid of most the caves while at the surface.
// Works by splitting the world into octants. The octant you're in decides which ways you can
// traverse the chunk grid by its relative position to the player. If you move up, you can't move
// down again, same for the other directions. So if you traverse up a mountain for example, you
// can't traverse down the other side again, so what's behind it is culled.
fn occlusion_culling(
    origin: Res<Origin>,
    settings: Res<Settings>,
    world_map: Res<WorldMap>,
    pause: Res<Pause>,
    mut chunk_query: Query<&mut Visibility, With<ChunkMarker>>,
    mut chunk_exists: Local<Vec<bool>>,
    mut chunk_visible: Local<Vec<bool>>,
) {
    if !origin.is_changed() {
        return;
    }

    if pause.0 {
        return;
    }

    // Matches the distance chunks are retained at by unload_chunks
    let radius = settings.render_distance as i32;
    let width = (radius * 2 + 1) as usize;

    chunk_exists.clear();
    chunk_exists.resize(width.pow(3), false);
    chunk_visible.clear();
    chunk_visible.resize(width.pow(3), false);

    let grid_index = |offset: IVec3| -> usize {
        (offset.x as usize * width + offset.y as usize) * width + offset.z as usize
    };
    let grid_offset = |chunk_position: IVec3| -> Option<IVec3> {
        let offset = (chunk_position - origin.0) / Chunk::SIZE as i32 + radius;
        if offset.cmplt(IVec3::ZERO).any() || offset.cmpge(IVec3::splat(width as i32)).any() {
            None
        } else {
            Some(offset)
        }
    };

    for chunk_position in world_map.chunks.keys() {
        if let Some(offset) = grid_offset(*chunk_position) {
            chunk_exists[grid_index(offset)] = true;
        }
    }

    let center = IVec3::splat(radius);
    chunk_visible[grid_index(center)] = chunk_exists[grid_index(center)];

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
        for dx in 0..=radius {
            for dy in 0..=radius {
                for dz in 0..=radius {
                    if dx == 0 && dy == 0 && dz == 0 {
                        continue;
                    }

                    let offset = center + IVec3::new(dx, dy, dz) * octant;
                    let index = grid_index(offset);
                    if !chunk_exists[index] {
                        continue;
                    }

                    chunk_visible[index] = (dx > 0
                        && chunk_visible[grid_index(offset - IVec3::new(octant.x, 0, 0))])
                        || (dy > 0
                            && chunk_visible[grid_index(offset - IVec3::new(0, octant.y, 0))])
                        || (dz > 0
                            && chunk_visible[grid_index(offset - IVec3::new(0, 0, octant.z))]);
                }
            }
        }
    }

    for (chunk_position, chunk) in world_map.chunks.iter() {
        let Some(entity) = chunk.entity() else {
            continue;
        };

        let visible = match grid_offset(*chunk_position) {
            Some(offset) => chunk_visible[grid_index(offset)],
            None => false,
        };

        if let Ok(mut visibility) = chunk_query.get_mut(entity) {
            visibility.set_if_neq(if visible {
                Visibility::Visible
            } else {
                Visibility::Hidden
            });
        }
    }
}

// TODO: This could take ResMut<Messages<ChunkResponse>> and drain the chunks to avoid
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
    blocks: Res<Blocks>,
    origin: Res<Origin>,
    mut world_map: ResMut<WorldMap>,
    mut new_chunk_events: MessageWriter<NewChunkEvent>,
    mut received_chunks: MessageReader<messages::Chunk>,
) {
    for chunk in received_chunks.read() {
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

        new_chunk_events.write(NewChunkEvent {
            position: chunk.position,
        });

        let entity =
            if chunk.blocks.len() == 1 && blocks.get_config(chunk.blocks[0]).quads.len() == 0 {
                // Uniform chunks that don't have a renderable block are ignored
                None
            } else {
                Some(
                    commands
                        .spawn((
                            Transform::from_translation((chunk.position - origin.0).as_vec3()),
                            Visibility::Visible,
                            MovesWithOrigin,
                            ChunkMarker,
                        ))
                        .id(),
                )
            };

        let old_chunk = world_map.insert(
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

        // This is just for the off chance where the server sends a chunk we already have because
        // of mismatch in render distance or execution order or whatever else.
        if let Some(entity) = old_chunk.and_then(|c| c.entity()) {
            commands.entity(entity).despawn();
        }
    }
}

// TODO: This doesn't feel like it belongs in this file
pub fn handle_block_updates(
    mut commands: Commands,
    net: Res<NetworkClient>,
    blocks: Res<Blocks>,
    origin: Res<Origin>,
    mut world_map: ResMut<WorldMap>,
    mut block_updates_events: MessageReader<messages::BlockUpdates>,
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
            let entity = commands
                .spawn((
                    Transform::from_translation((event.chunk_position - origin.0).as_vec3()),
                    Visibility::Visible,
                    MovesWithOrigin,
                    ChunkMarker,
                ))
                .id();
            chunk.convert_uniform_to_full(entity);
        }

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
