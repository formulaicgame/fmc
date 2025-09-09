use bevy::{math::DVec3, prelude::*};

use fmc_protocol::messages;

use crate::{
    bevy_extensions::f64_transform::{GlobalTransform, Transform},
    blocks::{BlockFace, BlockId, BlockPosition, BlockRotation, BlockState, Blocks},
    interfaces::InterfaceNodes,
    models::ModelMap,
    networking::{NetworkMessage, Server},
    physics::Collider,
    world::{ChunkOrigin, RenderDistance, WorldMap, chunk::ChunkPosition},
};

pub struct PlayersPlugin;
impl Plugin for PlayersPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            PreUpdate,
            (
                handle_player_position_updates,
                handle_camera_rotation_updates,
                find_target
                    .after(handle_player_position_updates)
                    .after(handle_camera_rotation_updates),
            ),
        );
    }
}

// TODO: Rename to Username. The struct name is useful, and it causes some patterns that are
// unsightly.
//
/// Player marker struct
#[derive(Component, Default)]
#[require(ChunkOrigin)]
pub struct Player {
    pub username: String,
}

// TODO: The reason for the awkward wrapping is wanting to have the camera be part of the player
// entity. Because of this it needs to be translated wherever it is used. Would be nice with a
// system that propagates it like with normal transforms.
//
/// Orientation of the player's camera.
/// The camera's translation is where the camera is relative to the player position.
#[derive(Component, Deref, DerefMut)]
pub struct Camera(Transform);

impl Camera {
    pub fn new(transform: Transform) -> Self {
        Self(transform)
    }

    /// Extract the camera's transform, prefer using Deref
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
pub(crate) struct DefaultPlayerBundle {
    player: Player,
    render_distance: RenderDistance,
    transform: Transform,
    camera: Camera,
    targets: Targets,
    collider: Collider,
    interfaces: InterfaceNodes,
}

impl DefaultPlayerBundle {
    pub fn new(username: String) -> Self {
        Self {
            player: Player { username },
            render_distance: RenderDistance { chunks: 1 },
            transform: Transform::default(),
            camera: Camera::default(),
            targets: Targets::default(),
            collider: Collider::from_min_max(
                DVec3::new(-0.3, 0.0, -0.3),
                DVec3::new(0.3, 1.8, 0.3),
            ),
            interfaces: InterfaceNodes::default(),
        }
    }
}

// TODO: Defunct, handled in wasm plugin now. Remember to remove the protocol message
fn send_aabb(
    net: Res<Server>,
    aabb_query: Query<(Entity, &Collider), (Changed<Collider>, With<Player>)>,
) {
    for (entity, collider) in aabb_query.iter() {
        let Collider::Single(aabb) = collider else {
            panic!();
        };

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
    mut player_query: Query<&mut Transform, With<Player>>,
    mut position_events: EventReader<NetworkMessage<messages::PlayerPosition>>,
) {
    for position_update in position_events.read() {
        if !position_update.position.is_finite() {
            continue;
        }

        let mut transform = player_query.get_mut(position_update.player_entity).unwrap();
        transform.translation = position_update.position;
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

/// Contains the [Target]'s the player is looking at, sorted by the distance from the camera.
/// The scan for targets will stop at the first entity it hits with an aabb or the first block that
/// is solid.
#[derive(Component, Deref, DerefMut, Debug, Default)]
pub struct Targets(Vec<Target>);

impl Targets {
    /// Get the first block that matches the provided condition
    pub fn get_first_block<F>(&self, f: F) -> Option<&Target>
    where
        F: Fn(&BlockId) -> bool,
    {
        for target in self.iter() {
            match target {
                Target::Entity { .. } => return None,
                Target::Block { block_id, .. } => {
                    if f(block_id) {
                        return Some(target);
                    }
                }
            }
        }

        return None;
    }
}
/// An object that can be targeted by a player
#[derive(Debug)]
pub enum Target {
    Entity {
        /// Distance to the target from the camera
        distance: f64,
        /// The face of the entity's aabb that was hit
        face: BlockFace,
        entity: Entity,
    },
    Block {
        block_position: BlockPosition,
        block_id: BlockId,
        /// Distance to the target from the camera
        distance: f64,
        /// The face of block that was hit
        block_face: BlockFace,
        /// The block's entity, if it has one
        entity: Option<Entity>,
    },
}

impl Target {
    /// Get the distance to the target
    pub fn distance(&self) -> f64 {
        match self {
            Self::Entity { distance, .. } => *distance,
            Self::Block { distance, .. } => *distance,
        }
    }

    /// Get the target's entity, if it has one
    pub fn entity(&self) -> Option<Entity> {
        match self {
            Target::Entity { entity, .. } => Some(*entity),
            Target::Block { entity, .. } => *entity,
        }
    }
}

const BLOCK_HIT_DISTANCE: f64 = 5.0;
const ENTITY_HIT_DISTANCE: f64 = 3.0;

fn find_target(
    world_map: Res<WorldMap>,
    model_map: Res<ModelMap>,
    model_query: Query<(
        Entity,
        Option<&Collider>,
        Option<&BlockPosition>,
        &GlobalTransform,
    )>,
    mut player_query: Query<(&mut Targets, &Camera, &Transform)>,
) {
    let blocks = Blocks::get();

    for (mut targets, camera, transform) in player_query.iter_mut() {
        targets.clear();

        let camera_transform = Transform {
            translation: transform.translation + camera.translation,
            rotation: camera.rotation,
            ..default()
        };

        let mut distance_to_solid_block = f64::MAX;

        let mut raycast = world_map.raycast(&camera_transform, BLOCK_HIT_DISTANCE);
        'outer: while let Some(block_id) = raycast.next_block() {
            let block_config = blocks.get_config(&block_id);

            // Block models are handled separately
            if block_config.model.is_some() {
                continue;
            }

            let block_position = raycast.position();
            let rotation = world_map
                .get_block_state(block_position)
                .map(BlockState::rotation)
                .flatten()
                .map(BlockRotation::as_quat)
                .unwrap_or_default();

            let block_transform = Transform {
                translation: block_position.as_dvec3() + DVec3::splat(0.5),
                rotation,
                ..default()
            };

            if let Some((distance, block_face)) = block_config
                .collider
                .ray_intersection(&block_transform, &camera_transform)
            {
                // TODO: it will add blocks with entities twice if the model is hit
                let chunk_position = ChunkPosition::from(block_position);
                let block_index = block_position.as_chunk_index();
                let entity = world_map
                    .get_chunk(&chunk_position)
                    .map(|chunk| chunk.block_entities.get(&block_index).cloned())
                    .flatten();

                distance_to_solid_block = distance;

                targets.push(Target::Block {
                    block_position,
                    block_id,
                    distance,
                    block_face,
                    entity,
                });

                if block_config.is_solid() {
                    break 'outer;
                }
            }
        }

        let chunk_position = ChunkPosition::from(transform.translation);
        for chunk_position in chunk_position.neighbourhood() {
            let Some(model_entities) = model_map.get_entities(&chunk_position) else {
                continue;
            };
            for (entity, maybe_collider, maybe_block, model_transform) in
                model_query.iter_many(model_entities)
            {
                if let Some(block_position) = maybe_block {
                    let Some(block_id) = world_map.get_block(*block_position) else {
                        continue;
                    };

                    let block_config = blocks.get_config(&block_id);

                    if let Some((distance, block_face)) = block_config
                        .collider
                        .ray_intersection(&model_transform.compute_transform(), &camera_transform)
                        && distance < distance_to_solid_block
                        && distance < BLOCK_HIT_DISTANCE
                    {
                        targets.push(Target::Block {
                            block_position: *block_position,
                            block_id,
                            block_face,
                            distance,
                            entity: Some(entity),
                        });
                        distance_to_solid_block = distance;
                    }
                } else if let Some(collider) = maybe_collider {
                    if let Some((distance, face)) = collider
                        .ray_intersection(&model_transform.compute_transform(), &camera_transform)
                        && distance < ENTITY_HIT_DISTANCE
                        && distance < distance_to_solid_block
                    {
                        targets.push(Target::Entity {
                            distance,
                            face,
                            entity,
                        })
                    };
                } else {
                    continue;
                };
            }
        }

        // Because the order in which things are added is arbitrary some targets might sneak in
        // that are past the block distance. For example we might add a static block first that is
        // actually occluded by a model block, but we only search for those later.
        targets.retain(|t| t.distance() <= distance_to_solid_block);
        targets.sort_unstable_by(|a, b| a.distance().partial_cmp(&b.distance()).unwrap());
    }
}
