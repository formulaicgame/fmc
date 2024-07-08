use bevy::{prelude::*, render::primitives::Aabb};
use fmc_networking::{messages, NetworkData};

use crate::{game_state::GameState, settings::Settings, world::MovesWithOrigin};

mod camera;
// TODO: This is pub because of asset loading, remove when redone
mod movement;
mod physics;

pub use camera::PlayerCameraMarker;

// Used at setup to set camera position and define the AABB, but should be changed by the server.
const DEFAULT_PLAYER_WIDTH: f32 = 0.6;
const DEFAULT_PLAYER_HEIGHT: f32 = 1.8;

/// Contains everything needed to add first-person fly camera behavior to your game
pub struct PlayerPlugin;
impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(movement::MovementPlugin)
            .add_plugins(camera::CameraPlugin)
            .add_systems(Startup, setup_player)
            .add_systems(Update, handle_aabb_update.run_if(GameState::in_game));
    }
}

// TODO: All this physics/control stuff has no business here. Server should send wasm plugin that
// does everything. This is needed for other types of movement too, like boats.
#[derive(Component, Default)]
pub struct PlayerState {
    // Current velocity
    pub velocity: Vec3,
    // Current acceleration
    pub acceleration: Vec3,
    pub is_flying: bool,
    pub is_swimming: bool,
    // If the player is against a block. (in any direction)
    pub is_grounded: BVec3,
}

impl PlayerState {
    pub fn new() -> Self {
        return Self {
            is_flying: true,
            ..Default::default()
        };
    }
}

fn setup_player(mut commands: Commands, settings: Res<Settings>) {
    let player = PlayerState::new();
    // XXX: This is replaced by the server, serves as a default
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
        .spawn((camera::CameraBundle::default(), settings.fog.clone()))
        .insert(SpatialListener::new(0.2))
        .id();

    let body = commands
        .spawn(player)
        .insert(VisibilityBundle::default())
        .insert(TransformBundle {
            local: Transform::from_translation(Vec3::NAN),
            ..default()
        })
        .insert(MovesWithOrigin)
        .insert(aabb)
        .id();

    commands.entity(body).push_children(&[head]);
}

fn handle_aabb_update(
    mut aabb_events: EventReader<NetworkData<messages::PlayerAabb>>,
    mut aabb_query: Query<&mut Aabb, With<PlayerState>>,
) {
    for aabb_event in aabb_events.read() {
        let mut aabb = aabb_query.single_mut();
        *aabb = Aabb {
            center: aabb_event.center.into(),
            half_extents: aabb_event.half_extents.into(),
        }
    }
}
