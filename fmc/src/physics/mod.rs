use std::collections::{HashMap, HashSet};

use bevy::math::{DQuat, DVec3};
use serde::{Deserialize, Serialize};

use crate::{
    blocks::{BlockFace, BlockPosition, BlockRotation, BlockState, Blocks},
    prelude::*,
    world::{ChangedBlockEvent, WorldMap, chunk::ChunkPosition},
};

pub mod shapes;

use self::shapes::{Aabb, AabbJson};

const GRAVITY: DVec3 = DVec3::new(0.0, -28.0, 0.0);

pub struct PhysicsPlugin;
impl Plugin for PhysicsPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Time::<Fixed>::from_seconds(1.0 / 60.0))
            .insert_resource(ObjectMap::default())
            .add_systems(Update, (update_object_map, trigger_update_on_block_change))
            .add_systems(
                FixedUpdate,
                (buoyancy, apply_acceleration, simulate_physics)
                    .chain()
                    .in_set(PhysicsSystems),
            );
    }
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum ColliderJson {
    Single(AabbJson),
    Vec(Vec<AabbJson>),
}

// TODO: Make Aabb available only through this? Either way need to replace all current occurences
#[derive(Component, Debug, Clone, Serialize)]
pub enum Collider {
    Single(Aabb),
    Multi(Vec<Aabb>),
}

impl Default for Collider {
    fn default() -> Self {
        Collider::Single(Aabb::default())
    }
}

impl Collider {
    /// Construct a collider from a set of aabb bounds.
    pub fn from_min_max(min: DVec3, max: DVec3) -> Self {
        Self::Single(Aabb::from_min_max(min, max))
    }

    pub fn as_aabb(&self) -> Aabb {
        match self {
            Self::Single(aabb) => *aabb,
            Self::Multi(aabbs) => {
                let mut min = DVec3::MAX;
                let mut max = DVec3::MIN;
                for aabb in aabbs {
                    min = min.min(aabb.min());
                    max = max.max(aabb.max());
                }

                Aabb::from_min_max(min, max)
            }
        }
    }

    /// Iterator over the block positions inside the collider
    fn iter_block_positions(
        &self,
        transform: &Transform,
    ) -> impl IntoIterator<Item = BlockPosition> {
        let aabb = self.as_aabb().transform(transform);

        let min = BlockPosition::from(aabb.min());
        let max = BlockPosition::from(aabb.max());
        (min.x..=max.x).flat_map(move |x| {
            (min.z..=max.z)
                .flat_map(move |z| (min.y..=max.y).map(move |y| BlockPosition::new(x, y, z)))
        })
    }

    fn iter(&self) -> std::slice::Iter<'_, Aabb> {
        match self {
            Collider::Single(aabb) => std::slice::from_ref(aabb).iter(),
            Collider::Multi(aabbs) => aabbs.iter(),
        }
    }

    /// Intersection test with another collider, returns the overlap if any.
    pub fn intersection(
        &self,
        self_transform: &Transform,
        other_transform: &Transform,
        other: &Collider,
    ) -> Option<DVec3> {
        let mut intersection = DVec3::ZERO;

        for left_aabb in self.iter() {
            let left_aabb = left_aabb.transform(self_transform);
            for right_aabb in other.iter().map(|aabb| aabb.transform(other_transform)) {
                if let Some(new_intersection) = left_aabb.intersection(&right_aabb) {
                    intersection = intersection
                        .abs()
                        .max(new_intersection.abs())
                        .copysign(new_intersection);
                }
            }
        }

        if intersection != DVec3::ZERO {
            return Some(intersection);
        } else {
            return None;
        }
    }

    /// Ray intersection test with the collider.
    pub fn ray_intersection(
        &self,
        self_transform: &Transform,
        ray_transform: &Transform,
    ) -> Option<(f64, BlockFace)> {
        match self {
            Self::Single(aabb) => aabb
                .transform(self_transform)
                .ray_intersection(ray_transform),
            Self::Multi(aabbs) => {
                let mut distance = f64::MAX;
                let mut face = BlockFace::Top;
                for aabb in aabbs {
                    if let Some((new_distance, new_face)) = aabb
                        .transform(self_transform)
                        .ray_intersection(ray_transform)
                    {
                        if new_distance < distance {
                            distance = new_distance;
                            face = new_face;
                        }
                    }
                }

                if distance != f64::MAX {
                    return Some((distance, face));
                } else {
                    return None;
                }
            }
        }
    }
}

/// For ordering systems to remove 1-frame lag
#[derive(Debug, Hash, PartialEq, Eq, Clone, SystemSet)]
pub struct PhysicsSystems;

/// Adds physics simulation to an entity, requires that you add a [Collider] to function.
#[derive(Component)]
pub struct Physics {
    pub enabled: bool,
    /// Set this to apply an impulse to the entity. It will reset each tick.
    pub acceleration: DVec3,
    /// The current velocity of the entity.
    pub velocity: DVec3,
    /// If the entity is currently blocked from moving along an axis.
    pub grounded: BVec3,
    /// Set this if the entity should be buoyant
    pub buoyancy: Option<Buoyancy>,
}

impl Default for Physics {
    fn default() -> Self {
        Self {
            enabled: true,
            acceleration: DVec3::default(),
            velocity: DVec3::default(),
            grounded: BVec3::FALSE,
            buoyancy: None,
        }
    }
}

/// Makes entities float
pub struct Buoyancy {
    /// Floats if this is lower than the block's Y-direction drag
    pub density: f64,
    /// Where on the entity's collider the waterline should sit.
    pub waterline: f64,
}

impl Default for Buoyancy {
    fn default() -> Self {
        Self {
            density: f64::MAX,
            waterline: 0.0,
        }
    }
}

// Keeps track of which entities are in which chunks. To efficiently trigger physics updates for a
// subset of entities when a chunk's blocks change.
#[derive(Resource, Default)]
struct ObjectMap {
    objects: HashMap<ChunkPosition, HashSet<Entity>>,
    reverse: HashMap<Entity, ChunkPosition>,
}

impl ObjectMap {
    fn get_entities(&self, chunk_position: &ChunkPosition) -> Option<&HashSet<Entity>> {
        return self.objects.get(chunk_position);
    }

    fn insert_or_move(&mut self, chunk_position: ChunkPosition, entity: Entity) {
        if let Some(current_chunk_pos) = self.reverse.get(&entity) {
            // Move model from one chunk to another
            if current_chunk_pos == &chunk_position {
                return;
            } else {
                let past_chunk_pos = self.reverse.remove(&entity).unwrap();

                self.objects
                    .get_mut(&past_chunk_pos)
                    .unwrap()
                    .remove(&entity);

                self.objects
                    .entry(chunk_position)
                    .or_insert(HashSet::new())
                    .insert(entity);

                self.reverse.insert(entity, chunk_position);
            }
        } else {
            // First time seeing model, insert it normally
            self.objects
                .entry(chunk_position)
                .or_insert(HashSet::new())
                .insert(entity);
            self.reverse.insert(entity, chunk_position);
        }
    }
}

// Moves all entities with collider along their velocity vector and resolves any collisions that
// occur with the environment.
fn simulate_physics(
    world_map: Res<WorldMap>,
    time: Res<Time<Fixed>>,
    mut entities: Query<(&mut Transform, &mut Physics, &Collider)>,
) {
    for (mut transform, mut physics, entity_collider) in entities.iter_mut() {
        if physics.velocity == DVec3::ZERO {
            continue;
        }

        if physics.velocity.x != 0.0 {
            physics.grounded.x = false;
        }
        if physics.velocity.y != 0.0 {
            physics.grounded.y = false;
        }
        if physics.velocity.z != 0.0 {
            physics.grounded.z = false;
        }

        let blocks = Blocks::get();

        let delta_time = DVec3::splat(time.delta_secs_f64());
        let mut new_position = transform.translation + physics.velocity * delta_time;
        let mut friction = DVec3::ZERO;
        let mut move_back = DVec3::ZERO;

        for velocity in [
            DVec3::new(0.0, physics.velocity.y, 0.0),
            DVec3::new(physics.velocity.x, 0.0, physics.velocity.z),
        ] {
            let pos_after_move =
                transform.with_translation(transform.translation + velocity * delta_time);

            for block_position in entity_collider.iter_block_positions(&pos_after_move) {
                let block_id = match world_map.get_block(block_position) {
                    Some(id) => id,
                    // If entity is player, disconnect? They should always have their
                    // surroundings loaded.
                    None => continue,
                };

                let block_config = blocks.get_config(&block_id);

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

                let Some(overlap) = entity_collider.intersection(
                    &pos_after_move,
                    &block_transform,
                    &block_config.collider,
                ) else {
                    continue;
                };

                if let Some(drag) = block_config.drag() {
                    friction = friction.max(drag);
                    continue;
                }

                let backwards_time = overlap / -velocity;
                // Small epsilon to delta time because of precision.
                let valid_axes = backwards_time.cmplt(delta_time + delta_time / 100.0)
                    & backwards_time.cmpgt(DVec3::ZERO);
                let resolution_axis =
                    DVec3::select(valid_axes, backwards_time, DVec3::NAN).max_element();

                if physics.grounded.y && overlap.y > 0.0 && overlap.y < 0.51 {
                    // This let's the player step up short distances when moving horizontally
                    move_back.y = move_back.y.max(0.05_f64.min(overlap.y + overlap.y / 100.0));
                    physics.grounded.y = true;
                    physics.velocity.y = 0.0;

                    if velocity.y.is_sign_positive() {
                        friction = friction.max(block_config.surface_friction(BlockFace::Bottom));
                    } else {
                        friction = friction.max(block_config.surface_friction(BlockFace::Top));
                    }
                } else if resolution_axis == backwards_time.y {
                    if physics.velocity.y.is_sign_positive() {
                        friction = friction.max(block_config.surface_friction(BlockFace::Bottom));
                    } else {
                        friction = friction.max(block_config.surface_friction(BlockFace::Top));
                    }

                    move_back.y = overlap.y + overlap.y / 100.0;
                    physics.velocity.y = 0.0;
                    physics.grounded.y = true;
                } else if resolution_axis == backwards_time.x {
                    if physics.velocity.x.is_sign_positive() {
                        friction = friction.max(block_config.surface_friction(BlockFace::Left));
                    } else {
                        friction = friction.max(block_config.surface_friction(BlockFace::Right));
                    }

                    move_back.x = overlap.x + overlap.x / 100.0;
                    physics.velocity.x = 0.0;
                    physics.grounded.x = true;
                } else if resolution_axis == backwards_time.z {
                    if physics.velocity.z.is_sign_positive() {
                        friction = friction.max(block_config.surface_friction(BlockFace::Back));
                    } else {
                        friction = friction.max(block_config.surface_friction(BlockFace::Front));
                    }

                    move_back.z = overlap.z + overlap.z / 100.0;
                    physics.velocity.z = 0.0;
                    physics.grounded.z = true;
                } else {
                    // When physics.velocity is really small there's numerical precision problems. Since a
                    // resolution is guaranteed. Move it back by whatever the smallest resolution
                    // direction is.
                    let valid_axes = DVec3::select(
                        backwards_time.cmpgt(DVec3::ZERO) & backwards_time.cmplt(delta_time * 10.0),
                        backwards_time,
                        DVec3::NAN,
                    );
                    if valid_axes.x.is_finite()
                        || valid_axes.y.is_finite()
                        || valid_axes.z.is_finite()
                    {
                        let valid_axes = DVec3::select(
                            valid_axes.cmpeq(DVec3::splat(valid_axes.min_element())),
                            valid_axes,
                            DVec3::ZERO,
                        );
                        move_back += (valid_axes + valid_axes / 100.0) * -velocity;
                    }
                }
            }
        }

        new_position += move_back;
        if (new_position - transform.translation)
            .abs()
            .cmpgt(DVec3::splat(0.0001))
            .any()
        {
            transform.translation = new_position;
        }

        // XXX: Pow(4) is just to scale it further towards zero when friction is high. The function
        // should be parsed as 'physics.velocity *= friction^time'
        physics.velocity =
            physics.velocity * (1.0 - friction).powf(4.0).powf(time.delta_secs_f64());
        // Clamp the physics.velocity when it is close to 0
        physics.velocity = DVec3::select(
            physics.velocity.abs().cmplt(DVec3::splat(0.01)),
            DVec3::ZERO,
            physics.velocity,
        );
    }
}

fn update_object_map(
    mut object_map: ResMut<ObjectMap>,
    object_query: Query<(Entity, &GlobalTransform), (With<Physics>, Changed<GlobalTransform>)>,
) {
    for (entity, transform) in object_query.iter() {
        let chunk_position = ChunkPosition::from(transform.translation());
        object_map.insert_or_move(chunk_position, entity)
    }
}

fn trigger_update_on_block_change(
    object_map: Res<ObjectMap>,
    mut object_query: Query<&mut Transform, With<Physics>>,
    mut block_updates: EventReader<ChangedBlockEvent>,
) {
    for block_update in block_updates.read() {
        let chunk_position = ChunkPosition::from(block_update.position);
        if let Some(item_entities) = object_map.get_entities(&chunk_position) {
            for entity in item_entities.iter() {
                if let Ok(mut transform) = object_query.get_mut(*entity) {
                    transform.set_changed();
                }
            }
        }

        let above_position = block_update.position + IVec3::Y;
        let above_chunk_position = ChunkPosition::from(above_position);
        if above_chunk_position != chunk_position {
            if let Some(item_entities) = object_map.get_entities(&above_chunk_position) {
                for entity in item_entities.iter() {
                    if let Ok(mut transform) = object_query.get_mut(*entity) {
                        transform.set_changed();
                    }
                }
            }
        }
    }
}

fn apply_acceleration(time: Res<Time<Fixed>>, mut objects: Query<&mut Physics>) {
    for mut physics in objects.iter_mut() {
        if physics.velocity == DVec3::ZERO && physics.acceleration == DVec3::ZERO {
            // Stationary objects are skipped until some external force is applied to them or a
            // block around them changes. Physics calculations are expensive.
            continue;
        }

        let acceleration = physics.acceleration + GRAVITY;

        physics.velocity += acceleration * time.delta_secs_f64();
        physics.acceleration = DVec3::ZERO;
    }
}

fn buoyancy(
    world_map: Res<WorldMap>,
    mut objects: Query<(&GlobalTransform, &mut Physics), Changed<GlobalTransform>>,
) {
    for (transform, mut physics) in objects.iter_mut() {
        let Some(buoyancy) = &mut physics.buoyancy else {
            continue;
        };

        let mut waterline_position = transform.translation();
        waterline_position.y += buoyancy.waterline;

        let block_position = BlockPosition::from(waterline_position);
        let Some(block_id) = world_map.get_block(block_position) else {
            continue;
        };
        let block_config = Blocks::get().get_config(&block_id);

        let Some(drag) = block_config.drag() else {
            continue;
        };

        // We want to let the object bob a little when it enters the water, but when it has
        // stabilized
        //let offset_from_top_of_block = 1.0 - (waterline_position.y - block_position.y as f64);
        if buoyancy.density < drag.y && waterline_position.y < block_position.y as f64 + 1.0 {
            //if offset_from_top_of_block < 0.05 {
            //    acceleration.0 += -GRAVITY;
            //} else {
            physics.acceleration += -GRAVITY + DVec3::new(0.0, 1.0, 0.0);
            //}
        }
    }
}
