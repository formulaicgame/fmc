use bevy::{math::DVec3, prelude::*, render::primitives::Aabb};
use fmc_networking::{messages, NetworkData};

use crate::{
    game_state::GameState,
    player::PlayerState,
    settings::Settings,
    world::{blocks::Blocks, world_map::WorldMap, Origin},
};

pub struct AudioPlugin;
impl Plugin for AudioPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ClientSideAudio { enabled: true })
            .add_systems(
                Update,
                (play_sounds, toggle_client_side_sound, play_walking_sound)
                    .run_if(GameState::in_game),
            );
    }
}

#[derive(Resource)]
struct ClientSideAudio {
    enabled: bool,
}

fn play_sounds(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    origin: Res<Origin>,
    mut sound_events: EventReader<NetworkData<messages::Sound>>,
) {
    for sound in sound_events.read() {
        let position = sound.position.unwrap_or(DVec3::ZERO) - origin.0.as_dvec3();
        commands
            .spawn(TransformBundle::from_transform(
                Transform::from_translation(position.as_vec3()),
            ))
            .insert(AudioBundle {
                source: asset_server.load(&sound.sound),
                settings: PlaybackSettings::DESPAWN.with_spatial(sound.position.is_some()),
            });
    }
}

fn toggle_client_side_sound(
    mut client_side_audio: ResMut<ClientSideAudio>,
    mut toggle_events: EventReader<NetworkData<messages::EnableClientAudio>>,
) {
    for event in toggle_events.read() {
        client_side_audio.enabled = event.0;
    }
}

// TODO: Try sending it from the server?
// Walking sound for the player is the only sound that is handled client side. For better responsivness.
fn play_walking_sound(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    origin: Res<Origin>,
    world_map: Res<WorldMap>,
    client_side_audio: Res<ClientSideAudio>,
    player_position: Query<
        (&GlobalTransform, &Aabb),
        (With<PlayerState>, Changed<GlobalTransform>),
    >,
    mut last_position: Local<DVec3>,
    mut distance: Local<f64>,
    mut last_sound_index: Local<usize>,
) {
    if !client_side_audio.enabled {
        return;
    }

    let Ok((global_transform, aabb)) = player_position.get_single() else {
        return;
    };

    let position = origin.0.as_dvec3() + global_transform.translation().as_dvec3();
    *distance += position.distance(*last_position).abs();
    *last_position = position;

    if *distance < 2.3 {
        return;
    }

    let mut center = aabb.half_extents;
    center.y = -0.05;
    // get block directly under player
    let block_position = (position + center.as_dvec3()).floor().as_ivec3();

    let Some(block_id) = world_map.get_block(&block_position) else {
        return;
    };

    let blocks = Blocks::get();
    let walking_sounds = blocks.get_config(block_id).walking_sounds();

    if walking_sounds.len() == 0 {
        return;
    }

    let mut index = std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_millis() as usize
        % walking_sounds.len().max(1);

    // Don't play the same sound twice
    if index == *last_sound_index {
        index += 1;
        index = index % walking_sounds.len().max(1);
    }

    *last_sound_index = index;
    *distance = 0.0;

    commands
        .spawn(TransformBundle::from_transform(
            Transform::from_translation(global_transform.translation() + Vec3::from(aabb.center)),
        ))
        .insert(AudioBundle {
            source: asset_server.load(&walking_sounds[index]),
            settings: PlaybackSettings::DESPAWN.with_spatial(false),
        });
}
