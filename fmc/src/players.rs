use bevy::{
    math::{DQuat, DVec3},
    prelude::*,
};

use fmc_networking::{
    messages, ConnectionId, NetworkData, NetworkServer, ServerNetworkEvent, Username,
};

use crate::{
    bevy_extensions::f64_transform::{GlobalTransform, Transform},
    interfaces::InterfaceNodes,
    models::{Model, ModelAnimations, ModelBundle, ModelVisibility, Models},
    physics::{shapes::Aabb, Velocity},
    world::RenderDistance,
};

pub struct PlayersPlugin;
impl Plugin for PlayersPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                add_player,
                send_aabb,
                handle_player_position_updates,
                handle_camera_rotation_updates,
                log_connections,
            ),
        );
    }
}

#[derive(Component, Default)]
pub struct Player;

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

/// Default bundle used for new players.
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

fn add_player(
    mut commands: Commands,
    models: Res<Models>,
    mut player_ready_events: EventReader<NetworkData<messages::ClientFinishedLoading>>,
) {
    for message in player_ready_events.read() {
        commands
            .entity(message.source.entity())
            .insert(DefaultPlayerBundle {
                player: Player,
                render_distance: RenderDistance::default(),
                global_transform: GlobalTransform::default(),
                transform: Transform::default(),
                camera: Camera::default(),
                velocity: Velocity::default(),
                aabb: Aabb::from_min_max(DVec3::new(-0.3, 0.0, -0.3), DVec3::new(0.3, 1.8, 0.3)),
                interfaces: InterfaceNodes::default(),
            })
            .with_children(|parent| {
                parent.spawn(ModelBundle {
                    model: Model {
                        id: models.get_by_name("player").id,
                    },
                    animations: ModelAnimations::default(),
                    visibility: ModelVisibility::default(),
                    global_transform: GlobalTransform::default(),
                    transform: Transform {
                        //translation: player_bundle.camera.translation - player_bundle.camera.translation.y,
                        translation: DVec3::Z * 0.3 + DVec3::X * 0.3,
                        ..default()
                    },
                });
            });
    }
}

fn log_connections(
    connection_query: Query<(&ConnectionId, &Username)>,
    mut network_events: EventReader<ServerNetworkEvent>,
) {
    for event in network_events.read() {
        match event {
            ServerNetworkEvent::Connected { entity } => {
                let (connection_id, username) = connection_query.get(*entity).unwrap();
                info!(
                    "Player connected, ip: {}, username: {}",
                    connection_id, username
                );
            }
            ServerNetworkEvent::Disconnected { entity } => {
                let (connection_id, username) = connection_query.get(*entity).unwrap();
                info!(
                    "Player disconnected, ip: {}, username: {}",
                    connection_id, username
                );
            }
            _ => {}
        }
    }
}

fn send_aabb(net: Res<NetworkServer>, aabb_query: Query<(&ConnectionId, &Aabb), Changed<Aabb>>) {
    for (connection, aabb) in aabb_query.iter() {
        net.send_one(
            *connection,
            messages::PlayerAabb {
                center: aabb.center.as_vec3(),
                half_extents: aabb.half_extents.as_vec3(),
            },
        );
    }
}

fn handle_player_position_updates(
    mut player_query: Query<(&mut Transform, &mut Velocity), With<Player>>,
    mut position_events: EventReader<NetworkData<messages::PlayerPosition>>,
) {
    for position_update in position_events.read() {
        let (mut player_position, mut player_velocity) = player_query
            .get_mut(position_update.source.entity())
            .unwrap();
        player_position.translation = position_update.position;
        player_velocity.0 = position_update.velocity;
    }
}

// Client sends the rotation of its camera. Used to know where they are looking, and
// how the player model should be positioned.
fn handle_camera_rotation_updates(
    mut player_query: Query<(&mut Camera, &Children)>,
    mut player_model_transforms: Query<&mut Transform, With<Model>>,
    mut camera_rotation_events: EventReader<NetworkData<messages::PlayerCameraRotation>>,
) {
    for rotation_update in camera_rotation_events.read() {
        let Ok((mut camera, children)) = player_query.get_mut(rotation_update.source.entity())
        else {
            // TODO: This guards against the client sending rotation updates too early. It
            // shouldn't be doing that, when the bug is found, make this a disconnect.
            continue;
        };
        camera.rotation = rotation_update.rotation.as_dquat();

        let mut transform = player_model_transforms
            .get_mut(*children.first().unwrap())
            .unwrap();
        let theta = camera.rotation.y.atan2(camera.rotation.w);
        transform.rotation = DQuat::from_xyzw(0.0, theta.sin(), 0.0, theta.cos());
    }
}
