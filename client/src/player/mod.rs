use bevy::{prelude::*, render::primitives::Aabb};
use fmc_protocol::messages;

use crate::{
    game_state::GameState,
    networking::NetworkClient,
    settings::Settings,
    world::{MovesWithOrigin, Origin},
};

mod camera;

// Used at setup to set camera position and define the AABB, but should be changed by the server.
const DEFAULT_PLAYER_WIDTH: f32 = 0.6;
const DEFAULT_PLAYER_HEIGHT: f32 = 1.8;

/// Contains everything needed to add first-person fly camera behavior to your game
pub struct PlayerPlugin;
impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(camera::CameraPlugin)
            .add_systems(Startup, setup_player)
            .add_systems(
                Update,
                (handle_aabb_update, send_position_to_server).run_if(in_state(GameState::Playing)),
            )
            // TODO: This is another one of the things the server just sends on connection.
            // Workaround by just having it run all the time, but once the server can be notified
            // that the client is actually ready to receive it should be moved above with the rest.
            .add_systems(Update, handle_position_updates_from_server);
    }
}

#[derive(Component)]
pub struct Player;

#[derive(Component)]
pub struct Head;

fn setup_player(mut commands: Commands, settings: Res<Settings>) {
    // This is replaced by the server, serves as a default
    let aabb = Aabb::from_min_max(
        Vec3::new(
            -DEFAULT_PLAYER_WIDTH / 2.0,
            0.0,
            -DEFAULT_PLAYER_WIDTH / 2.0,
        ),
        Vec3::new(
            DEFAULT_PLAYER_WIDTH / 2.0,
            DEFAULT_PLAYER_HEIGHT,
            DEFAULT_PLAYER_WIDTH / 2.0,
        ),
    );

    let head = commands
        .spawn((
            camera::camera_bundle(&settings),
            // TODO: This has to be inverted or the audio will be
            SpatialListener {
                left_ear_offset: Vec3::X * 0.2 / 2.0,
                right_ear_offset: Vec3::X * 0.2 / -2.0,
            },
            Head,
        ))
        .id();

    let body = commands
        .spawn((
            Player,
            Visibility::default(),
            Transform::from_translation(Vec3::NAN),
            MovesWithOrigin,
            aabb,
        ))
        .id();

    commands.entity(body).add_children(&[head]);
}

fn handle_aabb_update(
    mut aabb_events: EventReader<messages::PlayerAabb>,
    mut aabb_query: Query<&mut Aabb, With<Player>>,
) {
    for aabb_event in aabb_events.read() {
        let mut aabb = aabb_query.single_mut().unwrap();
        *aabb = Aabb {
            center: aabb_event.center.into(),
            half_extents: aabb_event.half_extents.into(),
        }
    }
}

fn handle_position_updates_from_server(
    origin: Res<Origin>,
    mut position_events: EventReader<messages::PlayerPosition>,
    mut player_query: Query<&mut Transform, With<Player>>,
) {
    for event in position_events.read() {
        let mut transform = player_query.single_mut().unwrap();
        transform.translation = origin.to_local(event.position);
    }
}

fn send_position_to_server(
    net: Res<NetworkClient>,
    origin: Res<Origin>,
    time: Res<Time>,
    player_transform: Query<&Transform, With<Player>>,
    mut last_time: Local<f32>,
    mut last_position: Local<Vec3>,
) {
    *last_time += time.delta_secs();
    if *last_time < 1.0 / 24.0 {
        // Fixed time is 1/144, but we want to send updates to the server at a more reasonable
        // cadence.
        return;
    }
    *last_time = 0.0;

    let transform = player_transform.single().unwrap();

    if last_position.distance_squared(transform.translation) < 0.00001 {
        return;
    }
    *last_position = transform.translation;

    net.send_message(messages::PlayerPosition {
        position: origin.to_global(transform.translation),
    });
}
