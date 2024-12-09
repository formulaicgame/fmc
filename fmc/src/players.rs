use bevy::{math::DVec3, prelude::*};

use fmc_protocol::messages;

use crate::{
    bevy_extensions::f64_transform::{GlobalTransform, Transform},
    blocks::{BlockFace, BlockPosition, Blocks},
    interfaces::InterfaceNodes,
    models::ModelMap,
    networking::{NetworkMessage, Server},
    physics::{shapes::Aabb, Velocity},
    utils,
    world::{chunk::Chunk, RenderDistance, WorldMap},
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
                find_target
                    .after(handle_player_position_updates)
                    .after(handle_camera_rotation_updates),
            ),
        );
    }
}

#[derive(Component, Default)]
pub struct Player {
    pub username: String,
}

// TODO: The reason for the awkward wrapping is wanting to have the camera be part of the player
// entity. Because of this it needs to be translated wherever it is used. Would be nice with a
// system that propagates it like with normal transforms.
//
/// Orientation of the player's camera.
/// The transform's translation is where the camera is relative to the player position.
#[derive(Component, Deref, DerefMut)]
pub struct Camera(Transform);

impl Camera {
    pub fn new(transform: Transform) -> Self {
        Self(transform)
    }

    pub fn transform(&self) -> &Transform {
        &self.0
    }
}

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
    target: Target,
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
            target: Target::None,
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

/// Tracks what the player is currently looking at
#[derive(Component, Debug)]
pub enum Target {
    Entity {
        /// Distance to the target from the camera
        distance: f64,
        /// The face of the entity's aabb that was hit
        face: BlockFace,
        entity: Entity,
    },
    Block {
        block_position: IVec3,
        /// Distance to the target from the camera
        distance: f64,
        /// The face of block that was hit
        block_face: BlockFace,
        /// The block's entity, if it has one
        entity: Option<Entity>,
    },
    None,
}

impl Target {
    fn set_to_closest(&mut self, other: Self) {
        let distance = self.distance();
        let other_distance = other.distance();

        if other_distance < distance {
            *self = other;
        }
    }

    fn distance(&self) -> f64 {
        match self {
            Self::Entity { distance, .. } => *distance,
            Self::Block { distance, .. } => *distance,
            Self::None => f64::MAX,
        }
    }
}

fn find_target(
    world_map: Res<WorldMap>,
    model_map: Res<ModelMap>,
    model_query: Query<(
        Entity,
        Option<&Aabb>,
        Option<&BlockPosition>,
        &GlobalTransform,
    )>,
    mut player_query: Query<
        (&mut Target, &Camera, &Transform),
        Or<(Changed<Camera>, Changed<Transform>)>,
    >,
) {
    let blocks = Blocks::get();

    for (mut target, camera, transform) in player_query.iter_mut() {
        *target = Target::None;

        let camera_transform = Transform {
            translation: transform.translation + camera.translation,
            rotation: camera.rotation,
            ..default()
        };

        let chunk_position =
            utils::world_position_to_chunk_position(transform.translation.floor().as_ivec3());
        // TODO: When ChunkPosition is implemented, this type of iteration should have its' own
        // function.
        for x_offset in [IVec3::X, IVec3::NEG_X, IVec3::ZERO] {
            for y_offset in [IVec3::Y, IVec3::NEG_Y, IVec3::ZERO] {
                for z_offset in [IVec3::Z, IVec3::NEG_Z, IVec3::ZERO] {
                    let chunk_position = chunk_position
                        + x_offset * Chunk::SIZE as i32
                        + y_offset * Chunk::SIZE as i32
                        + z_offset * Chunk::SIZE as i32;
                    let Some(model_entities) = model_map.get_entities(&chunk_position) else {
                        continue;
                    };
                    for (entity, maybe_aabb, maybe_block, model_transform) in
                        model_query.iter_many(model_entities)
                    {
                        let new_target = if let Some(block_position) = maybe_block {
                            let block_id = world_map.get_block(block_position.0).unwrap();
                            let block_config = blocks.get_config(&block_id);

                            let Some(hitbox) = &block_config.hitbox else {
                                continue;
                            };

                            let Some((distance, block_face)) = hitbox.ray_intersection(
                                &model_transform.compute_transform(),
                                &camera_transform,
                            ) else {
                                continue;
                            };

                            Target::Block {
                                block_position: block_position.0,
                                block_face,
                                distance,
                                entity: Some(entity),
                            }
                        } else if let Some(aabb) = maybe_aabb {
                            let Some((distance, face)) = aabb.ray_intersection(
                                &model_transform.compute_transform(),
                                &camera_transform,
                            ) else {
                                continue;
                            };

                            Target::Entity {
                                distance,
                                face,
                                entity,
                            }
                        } else {
                            continue;
                        };

                        target.set_to_closest(new_target);
                    }
                }
            }
        }

        if let Some((block_position, block_id, block_face, distance)) =
            world_map.raycast_to_block(&camera_transform, 5.0)
        {
            let new_target = Target::Block {
                block_position,
                distance,
                block_face,
                entity: None,
            };

            target.set_to_closest(new_target);
        }
    }
}
