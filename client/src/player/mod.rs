use bevy::{prelude::*, render::primitives::Aabb};
use fmc_protocol::messages;

use crate::{game_state::GameState, world::MovesWithOrigin};

mod camera;
mod movement;

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
            .add_systems(
                Update,
                handle_aabb_update.run_if(in_state(GameState::Playing)),
            );
    }
}

#[derive(Component)]
pub struct Head;

// TODO: All this physics/control stuff has no business here. Server should send wasm plugin that
// does everything. This is needed for other types of movement too, like boats.
#[derive(Component, Default)]
pub struct Player {
    // Current velocity
    pub velocity: Vec3,
    // Current acceleration
    pub acceleration: Vec3,
    pub is_flying: bool,
    pub is_swimming: bool,
    // If the player is against a block. (in any direction)
    pub is_grounded: BVec3,
}

impl Player {
    pub fn new() -> Self {
        return Self {
            is_flying: true,
            ..Default::default()
        };
    }
}

fn setup_player(mut commands: Commands) {
    let player = Player::new();
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
            camera::CameraBundle::default(),
            SpatialListener::new(0.2),
            Head,
        ))
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
    mut aabb_events: EventReader<messages::PlayerAabb>,
    mut aabb_query: Query<&mut Aabb, With<Player>>,
) {
    for aabb_event in aabb_events.read() {
        let mut aabb = aabb_query.single_mut();
        *aabb = Aabb {
            center: aabb_event.center.into(),
            half_extents: aabb_event.half_extents.into(),
        }
    }
}
