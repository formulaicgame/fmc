use std::time::Duration;

use bevy::{prelude::*, render::mesh::VertexAttributeValues};
use fmc_protocol::messages;

use crate::{
    game_state::GameState,
    networking::NetworkClient,
    player::{Head, Player},
    rendering::materials::ParticleMaterial,
    utils,
    world::{
        blocks::{Blocks, Friction},
        world_map::WorldMap,
        MovesWithOrigin, Origin,
    },
};

pub struct ParticlePlugin;
impl Plugin for ParticlePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            FixedUpdate,
            simulate_physics.run_if(in_state(GameState::Playing)),
        )
        .add_systems(
            Update,
            (
                handle_particles_from_server,
                despawn_particles,
                billboard_particles,
            )
                .run_if(in_state(GameState::Playing)),
        );
    }
}

#[derive(Component)]
struct Particle {
    lifetime: Timer,
}

impl Particle {
    fn new(lifetime: f32) -> Self {
        Self {
            lifetime: Timer::new(Duration::from_secs_f32(lifetime), TimerMode::Once),
        }
    }
}

fn handle_particles_from_server(
    mut commands: Commands,
    net: Res<NetworkClient>,
    origin: Res<Origin>,
    asset_server: Res<AssetServer>,
    mut new_effects: EventReader<messages::ParticleEffect>,
    mut rng: Local<utils::Rng>,
) {
    const TEXTURE_PATH: &str = "server_assets/active/textures/";

    for particle_effect in new_effects.read() {
        match particle_effect {
            messages::ParticleEffect::Explosion {
                position,
                spawn_offset,
                size_range,
                min_velocity,
                max_velocity,
                texture,
                color,
                lifetime,
                count,
            } => {
                for _ in 0..*count as usize {
                    let rand_offset = Vec3::new(rng.next_f32(), rng.next_f32(), rng.next_f32());
                    let offset = -*spawn_offset + *spawn_offset * 2.0 * rand_offset;
                    let translation = origin.to_local(*position) + offset;

                    let velocity = *min_velocity
                        + (*max_velocity - *min_velocity)
                            * Vec3::new(rng.next_f32(), rng.next_f32(), rng.next_f32());

                    let (min, max) = size_range;
                    let scale = Vec3::splat(min + (max - min) * rng.next_f32());

                    let mut mesh = Rectangle::from_length(0.5).mesh().build();

                    // Particles can be between 2 and 4 pixels
                    let particle_size = 2 + rng.next_u32() % 3;
                    // Choose a random location on the texture
                    let offset = (rng.next_u32() % (16 - particle_size)) as f32 / 16.0;
                    let uvs = mesh.attribute_mut(Mesh::ATTRIBUTE_UV_0).unwrap();
                    let VertexAttributeValues::Float32x2(uvs) = uvs else {
                        panic!();
                    };
                    for uv in uvs {
                        uv[0] = uv[0] * particle_size as f32 / 16.0 + offset;
                        uv[1] = uv[1] * particle_size as f32 / 16.0 + offset;
                    }

                    let base_color = if let Some(hex_color) = color {
                        let Ok(color) = Srgba::hex(hex_color) else {
                            net.disconnect(format!(
                                "Received malformed particle from server, '{}' is not a hex encoded color.", hex_color));
                            return;
                        };

                        color
                    } else {
                        Srgba::WHITE
                    };

                    let lifetime = lifetime.0 + (lifetime.1 - lifetime.0) * rng.next_f32();

                    commands.spawn((
                        Particle::new(lifetime),
                        Velocity(velocity),
                        Transform {
                            translation,
                            scale,
                            ..default()
                        },
                        Mesh3d(asset_server.add(mesh)),
                        MeshMaterial3d(
                            asset_server.add(ParticleMaterial {
                                texture: texture
                                    .as_ref()
                                    .map(|path| asset_server.load(TEXTURE_PATH.to_owned() + path)),
                                block_texture: texture
                                    .as_ref()
                                    .is_some_and(|path| path.starts_with("blocks")),
                                base_color,
                            }),
                        ),
                        MovesWithOrigin,
                    ));
                }
            }
        }
    }
}

// Spawn particles from particle spawners
// fn spawn_particles(
//     mut commands: Commands,
//     origin: Res<Origin>,
//     time: Res<Time>,
//     mut spawners: ResMut<Spawners>,
//     mut rng: Local<utils::Rng>,
// ) {
//     for spawner in spawners.values_mut() {
//         spawner.timer.tick(time.delta());
//         for _ in 0..spawner.timer.times_finished_this_tick() {
//             let rand_offset = Vec3::new(rng.next(), rng.next(), rng.next()).as_dvec3();
//             let offset = -spawner.offset + spawner.offset * 2.0 * rand_offset;
//             let translation = origin.to_translation(spawner.position + offset);
//
//             let min = spawner.min_velocity;
//             let max = spawner.max_velocity;
//             let velocity = min + (max - min) * rng.next();
//
//             let (min, max) = spawner.size_range;
//             let scale = Vec3::splat(min + (max - min) * rng.next());
//
//             commands.spawn((
//                 Particle::new(spawner.lifetime),
//                 physics::Velocity(velocity),
//                 MaterialMeshBundle {
//                     mesh: PARTICLE_MESH.clone(),
//                     material: spawner.material.clone(),
//                     transform: Transform {
//                         translation,
//                         scale,
//                         ..default()
//                     },
//                     ..default()
//                 },
//                 MovesWithOrigin,
//             ));
//         }
//     }
// }

fn despawn_particles(
    mut commands: Commands,
    time: Res<Time>,
    mut to_despawn: Query<(Entity, &mut Particle)>,
) {
    for (entity, mut particle) in to_despawn.iter_mut() {
        particle.lifetime.tick(time.delta());
        if particle.lifetime.finished() {
            commands.entity(entity).despawn();
        }
    }
}

// TODO: This can be done through shader, but it requires some transform matrix magic.
// Investigate:
// https://github.com/djeedai/bevy_hanabi/blob/19aee8dbccfa18bb0a298c9e8f2e8de6c4717c4c/src/modifier/output.rs#L481
// https://github.com/bevyengine/bevy/issues/3688
fn billboard_particles(
    camera_transform: Query<&GlobalTransform, With<Head>>,
    mut particles: Query<&mut Transform, (With<Particle>, Without<Player>)>,
) {
    let camera_transform = camera_transform.single();

    for mut particle_transform in particles.iter_mut() {
        // TODO: This should be "-camera_transform.forward()", maybe I did the mesh the wrong way
        // around, or it's a problem that it's missing normals.
        particle_transform.look_to(camera_transform.forward(), camera_transform.up());
    }
}

#[derive(Component, Default, Deref, DerefMut)]
pub struct Velocity(pub Vec3);

struct Aabb {
    center: Vec3,
    half_extents: Vec3,
}

impl Aabb {
    fn particle(transform: &Transform) -> Self {
        Self {
            center: transform.translation,
            half_extents: transform.scale * 0.5,
        }
    }

    fn block(position: Vec3) -> Self {
        Self {
            center: position + 0.5,
            half_extents: Vec3::splat(0.5),
        }
    }

    fn intersects(&self, other: &Aabb) -> Option<Vec3> {
        let distance = other.center - self.center;
        let overlap = self.half_extents + other.half_extents - distance.abs();

        if overlap.cmpgt(Vec3::ZERO).all() {
            Some(overlap.copysign(distance))
        } else {
            None
        }
    }

    fn min(&self) -> Vec3 {
        self.center - self.half_extents
    }

    fn max(&self) -> Vec3 {
        self.center + self.half_extents
    }
}

// BUG: Wanted to use Vec3A end to end, but the Vec3A::max_element function considers NaN to be
// greater than any number, where Vec3::max_element is opposite.
pub fn simulate_physics(
    origin: Res<Origin>,
    world_map: Res<WorldMap>,
    time: Res<Time>,
    mut entities: Query<(&mut Transform, &mut Velocity)>,
) {
    for (mut transform, mut velocity) in entities.iter_mut() {
        let gravity = Vec3::new(0.0, -14.0, 0.0);
        velocity.0 += gravity * time.delta_secs();

        let mut friction = Vec3::ZERO;

        for directional_velocity in [
            Vec3::new(0.0, velocity.y, 0.0),
            Vec3::new(velocity.x, 0.0, 0.0),
            Vec3::new(0.0, 0.0, velocity.z),
        ] {
            let mut particle_aabb = Aabb::particle(&transform);
            particle_aabb.center += directional_velocity * time.delta_secs();

            let blocks = Blocks::get();

            // Check for collisions with all blocks within the aabb.
            let mut collisions = Vec::new();
            let start = particle_aabb.min().floor().as_ivec3();
            let stop = particle_aabb.max().floor().as_ivec3();
            for x in start.x..=stop.x {
                for y in start.y..=stop.y {
                    for z in start.z..=stop.z {
                        let block_pos = origin.0 + IVec3::new(x, y, z);
                        // TODO: This looks up chunk through hashmap each time, is too bad?
                        let block_id = match world_map.get_block(&block_pos) {
                            Some(id) => id,
                            // If entity is player disconnect? They should always have their
                            // surroundings loaded.
                            None => continue,
                        };

                        let block_config = blocks.get_config(block_id);

                        friction = friction.max(block_config.drag());

                        // TODO: Implement colliders client side
                        let block_aabb = Aabb::block(Vec3::new(x as f32, y as f32, z as f32));

                        if let Some(overlap) = particle_aabb.intersects(&block_aabb) {
                            collisions.push((overlap, block_config));
                        }
                    }
                }
            }

            // TODO: This is remnant of when I tried to do all three axes at once. It could
            // probably be made to be simpler.
            let mut move_back = Vec3::ZERO;
            let delta_time = Vec3::splat(time.delta_secs());
            // Resolve the conflicts by moving the aabb the opposite way of the velocity vector on the
            // axis it takes the longest time to resolve the conflict.
            for (collision, block_config) in collisions {
                let backwards_time = collision / directional_velocity;
                // Small epsilon to delta time because of precision.
                let valid_axes = backwards_time.cmplt(delta_time + delta_time / 100.0)
                    & backwards_time.cmpgt(Vec3::ZERO);
                let resolution_axis = backwards_time.cmpeq(Vec3::splat(
                    Vec3::select(valid_axes, backwards_time, Vec3::NAN).max_element(),
                ));

                let Some(block_friction) = block_config.friction() else {
                    continue;
                };

                if resolution_axis.y {
                    if velocity.y.is_sign_positive() {
                        friction = friction.max(Vec3::splat(block_friction.bottom));
                    } else {
                        friction = friction.max(Vec3::splat(block_friction.top));
                    }

                    move_back.y = collision.y + collision.y / 100.0;
                    velocity.y = 0.0;
                } else if resolution_axis.x {
                    if velocity.x.is_sign_positive() {
                        friction = friction.max(Vec3::splat(block_friction.left));
                    } else {
                        friction = friction.max(Vec3::splat(block_friction.right));
                    }

                    move_back.x = collision.x + collision.x / 100.0;
                    velocity.x = 0.0;
                } else if resolution_axis.z {
                    if velocity.z.is_sign_positive() {
                        friction = friction.max(Vec3::splat(block_friction.back));
                    } else {
                        friction = friction.max(Vec3::splat(block_friction.front));
                    }

                    move_back.z = collision.z + collision.z / 100.0;
                    velocity.z = 0.0;
                } else {
                    // When velocity is really small there's numerical precision problems. Since a
                    // resolution is guaranteed. Move it back by whatever the smallest resolution
                    // direction is.
                    let valid_axes = Vec3::select(
                        backwards_time.cmpgt(Vec3::ZERO) & backwards_time.cmplt(delta_time * 2.0),
                        backwards_time,
                        Vec3::NAN,
                    );
                    if valid_axes.x.is_finite()
                        || valid_axes.y.is_finite()
                        || valid_axes.z.is_finite()
                    {
                        let valid_axes = Vec3::select(
                            valid_axes.cmpeq(Vec3::splat(valid_axes.min_element())),
                            valid_axes,
                            Vec3::ZERO,
                        );
                        move_back += (valid_axes + valid_axes / 100.0) * -directional_velocity;
                    }
                }
            }

            transform.translation = particle_aabb.center - move_back;
        }

        // XXX: Pow(4) is just to scale it further towards zero when friction is high. The function
        // should be understood as 'velocity *= friction^time'
        velocity.0 = velocity.0 * (1.0 - friction).powf(4.0).powf(time.delta_secs());
        // Clamp the velocity when it is close to 0
        velocity.0 = Vec3::select(
            velocity.0.abs().cmplt(Vec3::splat(0.01)),
            Vec3::ZERO,
            velocity.0,
        );
    }
}
