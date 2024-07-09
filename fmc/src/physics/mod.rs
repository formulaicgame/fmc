use std::collections::{HashMap, HashSet};

use bevy::{math::DVec3, prelude::*};

use crate::{
    bevy_extensions::f64_transform::{GlobalTransform, Transform},
    blocks::{Blocks, Friction},
    utils,
    world::{BlockUpdate, WorldMap},
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
                simulate_aabb_physics.in_set(PhysicsSystems),
                apply_acceleration.before(simulate_aabb_physics),
                gravity.before(apply_acceleration),
                buoyancy.before(apply_acceleration),
                update_object_map,
                trigger_update_on_block_change,
            ),
        );
    }
}

#[derive(Debug, Hash, PartialEq, Eq, Clone, SystemSet)]
pub struct PhysicsSystems;

/// Marker componenet, enables physics for an entity
#[derive(Component, Default)]
pub struct Mass;

#[derive(Component, Default, Deref, DerefMut)]
pub struct Acceleration(pub DVec3);

#[derive(Component, Default, Deref, DerefMut)]
pub struct Velocity(pub DVec3);

impl Velocity {
    pub fn is_moving(&self) -> bool {
        self.0 != DVec3::ZERO
    }
}

// Makes objects float (they sink by default)
#[derive(Component)]
pub struct Buoyancy {
    // Floats if this is lower than the block's Y-direction drag
    pub density: f64,
    // Where on the aabb the waterline should sit.
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

#[derive(Bundle, Default)]
pub struct PhysicsBundle {
    pub mass: Mass,
    pub accelection: Acceleration,
    pub velocity: Velocity,
    pub aabb: Aabb,
}

// Keeps track of which entities are in which chunks. To efficiently trigger physics updates for a
// subset of entities when a chunk's blocks change.
#[derive(Resource, Default)]
struct ObjectMap {
    objects: HashMap<IVec3, HashSet<Entity>>,
    reverse: HashMap<Entity, IVec3>,
}

impl ObjectMap {
    pub fn get_entities(&self, chunk_position: &IVec3) -> Option<&HashSet<Entity>> {
        return self.objects.get(chunk_position);
    }

    fn insert_or_move(&mut self, chunk_position: IVec3, entity: Entity) {
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
// Moves all entities with an aabb along their velocity vector and resolves any collisions that
// occur with the terrain.
fn simulate_aabb_physics(
    world_map: Res<WorldMap>,
    time: Res<Time>,
    mut entities: Query<(&mut Transform, &mut Velocity, &Aabb), With<Mass>>,
) {
    for (mut transform, mut velocity, aabb) in entities.iter_mut() {
        if velocity.0 == DVec3::ZERO {
            continue;
        }

        let mut friction = DVec3::ZERO;
        for directional_velocity in [
            DVec3::new(0.0, velocity.y, 0.0),
            DVec3::new(velocity.x, 0.0, 0.0),
            DVec3::new(0.0, 0.0, velocity.z),
        ] {
            let pos_after_move =
                transform.translation + directional_velocity * time.delta_seconds_f64();

            let entity_aabb = Aabb {
                center: aabb.center + pos_after_move,
                half_extents: aabb.half_extents,
            };

            let blocks = Blocks::get();

            // Check for collisions for all blocks within the aabb.
            let mut collisions = Vec::new();
            let start = entity_aabb.min().floor().as_ivec3();
            let stop = entity_aabb.max().floor().as_ivec3();
            for x in start.x..=stop.x {
                for y in start.y..=stop.y {
                    for z in start.z..=stop.z {
                        let block_pos = IVec3::new(x, y, z);
                        // TODO: This looks up chunk through hashmap each time, is too bad?
                        let block_id = match world_map.get_block(block_pos) {
                            Some(id) => id,
                            // If entity is player disconnect? They should always have their
                            // surroundings loaded.
                            None => continue,
                        };

                        let block_aabb = Aabb {
                            center: block_pos.as_dvec3() + 0.5,
                            half_extents: DVec3::new(0.5, 0.5, 0.5),
                        };

                        let distance = entity_aabb.center - block_aabb.center;
                        let overlap =
                            entity_aabb.half_extents + block_aabb.half_extents - distance.abs();

                        if overlap.cmpgt(DVec3::ZERO).all() {
                            //collisions.push((overlap, block_id));
                            collisions.push((DVec3::from(overlap.copysign(distance)), block_id));
                        }
                    }
                }
            }

            // TODO: This is remnant of when I tried to do all three axes at once. It could
            // probably be made to be simpler.
            let mut move_back = DVec3::ZERO;
            let delta_time = DVec3::splat(time.delta_seconds_f64());
            // Resolve the conflicts by moving the aabb the opposite way of the velocity vector on the
            // axis it takes the longest time to resolve the conflict.
            for (collision, block_id) in collisions {
                let backwards_time = collision / -directional_velocity;
                // Small epsilon to delta time because of precision.
                let valid_axes = backwards_time.cmplt(delta_time + delta_time / 100.0)
                    & backwards_time.cmpgt(DVec3::ZERO);
                let resolution_axis =
                    DVec3::select(valid_axes, backwards_time, DVec3::NAN).max_element();

                match blocks.get_config(&block_id).friction {
                    Friction::Static {
                        front,
                        back,
                        right,
                        left,
                        top,
                        bottom,
                    } => {
                        if resolution_axis == backwards_time.y {
                            if velocity.y.is_sign_positive() {
                                friction = friction.max(DVec3::splat(bottom));
                            } else {
                                friction = friction.max(DVec3::splat(top));
                            }

                            move_back.y = collision.y + collision.y / 100.0;
                            velocity.y = 0.0;
                        } else if resolution_axis == backwards_time.x {
                            if velocity.x.is_sign_positive() {
                                friction = friction.max(DVec3::splat(left));
                            } else {
                                friction = friction.max(DVec3::splat(right));
                            }

                            move_back.x = collision.x + collision.x / 100.0;
                            velocity.x = 0.0;
                        } else if resolution_axis == backwards_time.z {
                            if velocity.z.is_sign_positive() {
                                friction = friction.max(DVec3::splat(back));
                            } else {
                                friction = friction.max(DVec3::splat(front));
                            }

                            move_back.z = collision.z + collision.z / 100.0;
                            velocity.z = 0.0;
                        } else {
                            // When velocity is really small there's numerical precision problems. Since a
                            // resolution is guaranteed. Move it back by whatever the smallest resolution
                            // direction is.
                            let valid_axes = DVec3::select(
                                backwards_time.cmpgt(DVec3::ZERO)
                                    & backwards_time.cmplt(delta_time * 2.0),
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
                                move_back +=
                                    (valid_axes + valid_axes / 100.0) * -directional_velocity;
                            }
                        }
                    }
                    Friction::Drag(drag) => {
                        friction = friction.max(drag);
                    }
                }
            }

            if (transform.translation - (pos_after_move + move_back))
                .abs()
                .cmpgt(DVec3::splat(0.0001))
                .any()
            {
                transform.translation = pos_after_move + move_back;
            }
        }

        // XXX: Pow(4) is just to scale it further towards zero when friction is high. The function
        // should be parsed as 'velocity *= friction^time'
        velocity.0 = velocity.0 * (1.0 - friction).powf(4.0).powf(time.delta_seconds_f64());
        // Clamp the velocity when it is close to 0
        velocity.0 = DVec3::select(
            velocity.0.abs().cmplt(DVec3::splat(0.01)),
            DVec3::ZERO,
            velocity.0,
        );
    }
}

fn update_object_map(
    mut object_map: ResMut<ObjectMap>,
    object_query: Query<(Entity, &GlobalTransform), (With<Mass>, Changed<GlobalTransform>)>,
) {
    for (entity, global_transform) in object_query.iter() {
        let transform = global_transform.compute_transform();
        let chunk_position =
            utils::world_position_to_chunk_position(transform.translation.as_ivec3());
        object_map.insert_or_move(chunk_position, entity)
    }
}

fn trigger_update_on_block_change(
    object_map: Res<ObjectMap>,
    mut object_query: Query<&mut Transform, With<Mass>>,
    mut block_updates: EventReader<BlockUpdate>,
) {
    for block_update in block_updates.read() {
        let position = match block_update {
            BlockUpdate::Change { position, .. } => *position,
            _ => continue,
        };
        let chunk_position = utils::world_position_to_chunk_position(position);
        if let Some(item_entities) = object_map.get_entities(&chunk_position) {
            for entity in item_entities.iter() {
                if let Ok(mut transform) = object_query.get_mut(*entity) {
                    transform.set_changed();
                }
            }
        }

        let above_position = position + IVec3::Y;
        let above_chunk_position = utils::world_position_to_chunk_position(above_position);
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

fn apply_acceleration(
    time: Res<Time>,
    mut objects: Query<(Ref<GlobalTransform>, &mut Acceleration, &mut Velocity), With<Mass>>,
) {
    for (transform, mut acceleration, mut velocity) in objects.iter_mut() {
        if !transform.is_changed() && acceleration.0 == DVec3::ZERO && velocity.0 == DVec3::ZERO {
            // If the transform isn't modified and the object has no acceleration and
            // velocity it is considered stationary. Stationary objects are skipped until some
            // external force is applied to them.
            continue;
        }
        velocity.0 += acceleration.0 * time.delta_seconds_f64();
        acceleration.0 = DVec3::ZERO;
    }
}

fn gravity(mut objects: Query<&mut Acceleration, (With<Mass>, Changed<GlobalTransform>)>) {
    for mut acceleration in objects.iter_mut() {
        acceleration.0 += GRAVITY;
    }
}

fn buoyancy(
    world_map: Res<WorldMap>,
    mut objects: Query<
        (&GlobalTransform, &mut Acceleration, &Buoyancy),
        (With<Mass>, Changed<GlobalTransform>),
    >,
) {
    for (transform, mut acceleration, buoyancy) in objects.iter_mut() {
        let mut waterline_position = transform.translation();
        waterline_position.y += buoyancy.waterline;

        let block_position = waterline_position.floor().as_ivec3();
        let Some(block_id) = world_map.get_block(block_position) else {
            continue;
        };
        let block_config = Blocks::get().get_config(&block_id);

        let friction = match block_config.friction {
            Friction::Static { .. } => continue,
            Friction::Drag(f) => f,
        };

        // We want to let the object bob a little when it enters the water, but when it has
        // stabilized
        //let offset_from_top_of_block = 1.0 - (waterline_position.y - block_position.y as f64);
        if buoyancy.density < friction.y && waterline_position.y < block_position.y as f64 + 1.0 {
            //if offset_from_top_of_block < 0.05 {
            //    acceleration.0 += -GRAVITY;
            //} else {
            acceleration.0 += -GRAVITY + DVec3::new(0.0, 1.0, 0.0);
            //}
        }
    }
}
