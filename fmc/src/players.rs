use bevy::{math::DVec3, prelude::*};

use fmc_protocol::messages;

use crate::{
    bevy_extensions::f64_transform::{GlobalTransform, Transform},
    interfaces::InterfaceNodes,
    networking::{NetworkMessage, Server},
    physics::{shapes::Aabb, Velocity},
    world::RenderDistance,
};

pub struct PlayersPlugin;
impl Plugin for PlayersPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                send_aabb,
                handle_player_position_updates,
                handle_camera_rotation_updates,
            ),
        );
    }
}

#[derive(Component, Default)]
pub struct Player {
    pub username: String,
}

/// Orientation of the player's camera.
/// The transform's translation is where the camera is relative to the player position.
#[derive(Component, Deref, DerefMut)]
pub struct Camera(pub Transform);

impl Default for Camera {
    fn default() -> Self {
        Self(Transform {
            translation: DVec3::new(0.0, 1.65, 0.0),
            ..default()
        })
    }
}

#[derive(Bundle)]
pub struct DefaultPlayerBundle {
    player: Player,
    render_distance: RenderDistance,
    global_transform: GlobalTransform,
    transform: Transform,
    velocity: Velocity,
    camera: Camera,
    aabb: Aabb,
    interfaces: InterfaceNodes,
}

impl DefaultPlayerBundle {
    pub fn new(username: String) -> Self {
        Self {
            player: Player { username },
            render_distance: RenderDistance { chunks: 1 },
            global_transform: GlobalTransform::default(),
            transform: Transform::default(),
            camera: Camera::default(),
            velocity: Velocity::default(),
            aabb: Aabb::from_min_max(DVec3::new(-0.3, 0.0, -0.3), DVec3::new(0.3, 1.8, 0.3)),
            interfaces: InterfaceNodes::default(),
        }
    }
}

fn send_aabb(net: Res<Server>, aabb_query: Query<(Entity, &Aabb), (Changed<Aabb>, With<Player>)>) {
    for (entity, aabb) in aabb_query.iter() {
        net.send_one(
            entity,
            messages::PlayerAabb {
                center: aabb.center.as_vec3(),
                half_extents: aabb.half_extents.as_vec3(),
            },
        );
    }
}

fn handle_player_position_updates(
    mut player_query: Query<(&mut Transform, &mut Velocity), With<Player>>,
    mut position_events: EventReader<NetworkMessage<messages::PlayerPosition>>,
) {
    for position_update in position_events.read() {
        if !position_update.position.is_finite() {
            continue;
        }

        let (mut player_position, mut player_velocity) =
            player_query.get_mut(position_update.player_entity).unwrap();
        player_position.translation = position_update.position;
        player_velocity.0 = position_update.velocity;
    }
}

// Client sends the rotation of its camera. Used to know where they are looking, and
// how the player model should be positioned.
fn handle_camera_rotation_updates(
    mut player_query: Query<&mut Camera>,
    mut camera_rotation_events: EventReader<NetworkMessage<messages::PlayerCameraRotation>>,
) {
    for rotation_update in camera_rotation_events.read() {
        let mut camera = player_query.get_mut(rotation_update.player_entity).unwrap();
        camera.rotation = rotation_update.rotation.as_dquat();
    }
}
