use bevy::{audio::Volume, math::DVec3, prelude::*, render::primitives::Aabb};
use fmc_protocol::messages;

use crate::{
    game_state::GameState,
    player::Player,
    world::{blocks::Blocks, world_map::WorldMap, Origin},
};

const AUDIO_PATH: &str = "server_assets/active/audio/";

pub struct AudioPlugin;
impl Plugin for AudioPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ClientSideAudio { enabled: true })
            .add_systems(
                Update,
                (play_sounds, toggle_client_side_sound, play_walking_sound)
                    .run_if(in_state(GameState::Playing)),
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
    mut sound_events: EventReader<messages::Sound>,
) {
    for sound in sound_events.read() {
        commands.spawn((
            Transform::from_translation(origin.to_local(sound.position.unwrap_or(DVec3::ZERO))),
            AudioPlayer::<AudioSource>(asset_server.load(AUDIO_PATH.to_owned() + &sound.sound)),
            PlaybackSettings::DESPAWN
                .with_spatial(sound.position.is_some())
                .with_speed(sound.speed)
                .with_volume(Volume::new(sound.volume.clamp(0.0, 1.0))),
        ));
    }
}

fn toggle_client_side_sound(
    mut client_side_audio: ResMut<ClientSideAudio>,
    mut toggle_events: EventReader<messages::EnableClientAudio>,
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
    player_position: Query<(&GlobalTransform, &Aabb), (With<Player>, Changed<GlobalTransform>)>,
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

    let position = origin.to_global(global_transform.translation());
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
    let step_sounds = blocks.get_config(block_id).step_sounds();

    if step_sounds.len() == 0 {
        return;
    }

    let mut index = std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_millis() as usize
        % step_sounds.len();

    // Don't play the same sound twice
    if index == *last_sound_index {
        index += 1;
        index = index % step_sounds.len().max(1);
    }

    *last_sound_index = index;
    *distance = 0.0;

    commands.spawn((
        Transform::from_translation(global_transform.translation() + Vec3::from(aabb.center)),
        AudioPlayer::<AudioSource>(asset_server.load(AUDIO_PATH.to_owned() + &step_sounds[index])),
        PlaybackSettings::DESPAWN
            .with_spatial(false)
            .with_volume(Volume::new(0.1)),
    ));
}
