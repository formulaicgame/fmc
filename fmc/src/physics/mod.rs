use std::collections::{HashMap, HashSet};

use bevy::math::DVec3;
use serde::Deserialize;

use crate::{
    blocks::{BlockFace, BlockPosition, Blocks},
    prelude::*,
    world::{chunk::ChunkPosition, ChangedBlockEvent, WorldMap},
};

pub mod shapes;

use self::shapes::Aabb;

const GRAVITY: DVec3 = DVec3::new(0.0, -28.0, 0.0);

pub struct PhysicsPlugin;
impl Plugin for PhysicsPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ObjectMap::default()).add_systems(
            Update,
            (
                simulate_physics.in_set(PhysicsSystems),
                apply_acceleration.before(simulate_physics),
                buoyancy.before(apply_acceleration),
                update_object_map,
                trigger_update_on_block_change,
            ),
        );
    }
}

// TODO: Make Aabb available only through this? Either way need to replace all current occurences
#[derive(Component, Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum Collider {
    Aabb(Aabb),
    Compound(Vec<Aabb>),
}

impl Collider {
    // pub fn transform(&self, transform: &Transform) -> Self {
    //     let mut new = self.clone();
    //     for aabb in new.iter_mut() {
    //         aabb.transf
    //     }
    //     match self {
    //         Self::Aabb(aabb) => Collider::Aabb(aabb.transform(transform)),
    //         Self::Compound(aabbs) => {
    //             for aabb in aabbs {
    //                 aabb.center = transform.transform_point(aabb.center)
    //             }
    //         }
    //     }
    // }

    /// Construct a collider from a set of aabb bounds.
    pub fn from_min_max(min: DVec3, max: DVec3) -> Self {
        Self::Aabb(Aabb::from_min_max(min, max))
    }

    /// Get the minimum and maximum bounds of the collider.
    pub fn min_max(&self, transform: &Transform) -> (DVec3, DVec3) {
        let mut min = DVec3::MAX;
        let mut max = DVec3::MIN;
        for aabb in self.iter() {
            let aabb = aabb.transform(transform);
            min = min.min(aabb.min());
            max = max.max(aabb.max());
        }
        (min, max)
    }

    /// Iterate over the shapes of the collider
    fn iter(&self) -> &[Aabb] {
        match self {
            Self::Aabb(aabb) => std::slice::from_ref(aabb),
            Self::Compound(aabbs) => aabbs.as_slice(),
        }
    }

    // fn iter_mut(&mut self) -> &mut [Aabb] {
    //     match self {
    //         Self::Aabb(aabb) => std::slice::from_mut(aabb),
    //         Self::Compound(aabbs) => aabbs.as_mut_slice(),
    //     }
    // }

    /// Intersection test with another collider, returns the overlap if any.
    pub fn intersection(
        &self,
        self_transform: &Transform,
        other: &Collider,
        other_transform: &Transform,
    ) -> Option<DVec3> {
        fn max_intersection(lhs: Option<DVec3>, rhs: Option<DVec3>) -> Option<DVec3> {
            let Some(lhs) = lhs else {
                return rhs;
            };

            let Some(rhs) = rhs else {
                return Some(lhs);
            };

            // TODO: I don't know how to properly handle the sign here. The idea of combining the
            // intersections might not even be possible. This will certainly not work if the aabbs
            // are more than a negligible distance from each other.
            Some(lhs.abs().max(rhs.abs()).copysign(lhs))
        }

        let mut intersection = None;
        for aabb in self.iter() {
            let aabb = aabb.transform(self_transform);
            for other_aabb in other.iter() {
                let other_aabb = other_aabb.transform(other_transform);
                let new_intersection = aabb.intersection(&other_aabb);
                intersection = max_intersection(intersection, new_intersection);
            }
        }

        return intersection;
    }

    /// Ray intersection test with the collider.
    pub fn ray_intersection(
        &self,
        collider_transform: &Transform,
        ray_transform: &Transform,
    ) -> Option<(f64, BlockFace)> {
        match self {
            Self::Aabb(aabb) => aabb.ray_intersection(collider_transform, ray_transform),
            Self::Compound(aabbs) => {
                let mut hit = None;
                let mut distance = f64::MAX;
                for aabb in aabbs {
                    if let Some((new_distance, new_face)) =
                        aabb.ray_intersection(collider_transform, ray_transform)
                    {
                        if new_distance < distance {
                            hit = Some((new_distance, new_face));
                            distance = new_distance
                        }
                    }
                }
                hit
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

// BUG: Wanted to use Vec3A end to end, but the Vec3A::max_element function considers NaN to be
// greater than any number, where Vec3::max_element is opposite.
//
// Moves all entities with collider along their velocity vector and resolves any collisions that
// occur with the environment.
fn simulate_physics(
    world_map: Res<WorldMap>,
    time: Res<Time>,
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

        let mut friction = DVec3::ZERO;

        for directional_velocity in [
            DVec3::new(0.0, physics.velocity.y, 0.0),
            DVec3::new(physics.velocity.x, 0.0, physics.velocity.z),
        ] {
            let pos_after_move = transform.with_translation(
                transform.translation + directional_velocity * time.delta_secs_f64(),
            );

            // TODO: Allocation is unnecessary
            // Check for collisions with all blocks within the aabb.
            let mut collisions = Vec::new();
            let (min, max) = entity_collider.min_max(&pos_after_move);
            let start = BlockPosition::from(min);
            let stop = BlockPosition::from(max);
            for x in start.x..=stop.x {
                for y in start.y..=stop.y {
                    for z in start.z..=stop.z {
                        let block_pos = BlockPosition::new(x, y, z);
                        // TODO: This looks up chunk through hashmap each time, is too bad?
                        let block_id = match world_map.get_block(block_pos) {
                            Some(id) => id,
                            // If entity is player disconnect? They should always have their
                            // surroundings loaded.
                            None => continue,
                        };

                        let block_config = blocks.get_config(&block_id);

                        friction = friction.max(block_config.drag);

                        let block_collider = match &block_config.hitbox {
                            Some(c) => c,
                            None => continue,
                        };

                        if let Some(intersection) = entity_collider.intersection(
                            &pos_after_move,
                            &block_collider,
                            &Transform::from_translation(
                                block_pos.as_dvec3() + DVec3::new(0.5, 0.0, 0.5),
                            ),
                        ) {
                            collisions.push((intersection, block_config));
                        } else {
                            continue;
                        };
                    }
                }
            }

            // TODO: This is remnant of when I tried to do all three axes at once. It could
            // probably be made to be simpler.
            let mut move_back = DVec3::ZERO;
            let delta_time = DVec3::splat(time.delta_secs_f64());
            // Resolve the conflicts by moving the aabb the opposite way of the velocity vector on the
            // axis it takes the longest time to resolve the conflict.
            for (collision, block_config) in collisions {
                let backwards_time = collision / -directional_velocity;
                // Small epsilon to delta time because of precision.
                let valid_axes = backwards_time.cmplt(delta_time + delta_time / 100.0)
                    & backwards_time.cmpgt(DVec3::ZERO);
                let resolution_axis =
                    DVec3::select(valid_axes, backwards_time, DVec3::NAN).max_element();

                let Some(block_friction) = &block_config.friction else {
                    continue;
                };

                if resolution_axis == backwards_time.y {
                    if physics.velocity.y.is_sign_positive() {
                        friction = friction.max(DVec3::splat(block_friction.bottom));
                    } else {
                        friction = friction.max(DVec3::splat(block_friction.top));
                    }

                    move_back.y = collision.y + collision.y / 100.0;
                    physics.velocity.y = 0.0;
                    physics.grounded.y = true;
                } else if resolution_axis == backwards_time.x {
                    if physics.velocity.x.is_sign_positive() {
                        friction = friction.max(DVec3::splat(block_friction.left));
                    } else {
                        friction = friction.max(DVec3::splat(block_friction.right));
                    }

                    move_back.x = collision.x + collision.x / 100.0;
                    physics.velocity.x = 0.0;
                    physics.grounded.x = true;
                } else if resolution_axis == backwards_time.z {
                    if physics.velocity.z.is_sign_positive() {
                        friction = friction.max(DVec3::splat(block_friction.back));
                    } else {
                        friction = friction.max(DVec3::splat(block_friction.front));
                    }

                    move_back.z = collision.z + collision.z / 100.0;
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
                        move_back += (valid_axes + valid_axes / 100.0) * -directional_velocity;
                    }
                }
            }

            if (transform.translation - (pos_after_move.translation + move_back))
                .abs()
                .cmpgt(DVec3::splat(0.0001))
                .any()
            {
                transform.translation = pos_after_move.translation + move_back;
            }
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

fn apply_acceleration(time: Res<Time>, mut objects: Query<(Ref<GlobalTransform>, &mut Physics)>) {
    for (transform, mut physics) in objects.iter_mut() {
        if !transform.is_changed() {
            // If the transform isn't modified it is considered stationary. Stationary objects are
            // skipped until some external force is applied to them or a block around them changes.
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

        if block_config.is_solid() {
            continue;
        }

        // We want to let the object bob a little when it enters the water, but when it has
        // stabilized
        //let offset_from_top_of_block = 1.0 - (waterline_position.y - block_position.y as f64);
        if buoyancy.density < block_config.drag.y
            && waterline_position.y < block_position.y as f64 + 1.0
        {
            //if offset_from_top_of_block < 0.05 {
            //    acceleration.0 += -GRAVITY;
            //} else {
            physics.acceleration += -GRAVITY + DVec3::new(0.0, 1.0, 0.0);
            //}
        }
    }
}
