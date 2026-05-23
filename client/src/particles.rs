use std::time::Duration;

use bevy::{camera::primitives::Aabb, math::Vec3A, mesh::MeshTag, prelude::*};
use fmc_protocol::messages;

use crate::{
    assets::particles::{ParticleEffects, ParticleTextures},
    blocks::{BlockRotation, Blocks},
    game_state::GameState,
    player::{Head, Player},
    rendering::materials::ParticleMaterial,
    utils,
    world::{MovesWithOrigin, Origin, WorldMap},
};

pub struct ParticlePlugin;
impl Plugin for ParticlePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            FixedUpdate,
            simulate_particle_physics.run_if(in_state(GameState::Playing)),
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
    acceleration: Vec3,
    collision: bool,
    friction: Vec3,
}

impl Particle {
    fn new(lifetime: f32, acceleration: Vec3, collision: bool, friction: Vec3) -> Self {
        Self {
            lifetime: Timer::new(Duration::from_secs_f32(lifetime), TimerMode::Once),
            acceleration,
            collision,
            friction,
        }
    }
}

fn handle_particles_from_server(
    mut commands: Commands,
    time: Res<Time>,
    origin: Res<Origin>,
    asset_server: Res<AssetServer>,
    particle_effects: Res<ParticleEffects>,
    particle_textures: Res<ParticleTextures>,
    mut new_effects: MessageReader<messages::ParticleEffect>,
    mut rng: Local<utils::Rng>,
) {
    for event in new_effects.read() {
        let Some(effect) = particle_effects.get(&event.id) else {
            warn!("Received invalid particle effect id: {}", event.id);
            continue;
        };

        let Some(texture) = particle_textures.get(&event.texture).cloned() else {
            warn!(
                "Received invalid particle texture, no texture at: {}",
                event.texture
            );
            continue;
        };

        let material = asset_server.add(ParticleMaterial {
            texture,
            base_color: Srgba::from_vec4(event.color),
            lifetime: Vec2::from_array(effect.lifetime),
            random_uv: effect.random_uv.unwrap_or_default(),
            spawn_time: time.elapsed_secs(),
        });

        let mesh = asset_server.add(Rectangle::from_length(0.5).mesh().build());

        let position = origin.to_local(event.position);
        for _ in 0..effect.count {
            let local_position = effect.position(&mut rng);
            let local_velocity = effect.velocity(&mut rng, local_position);
            let translation = position + event.rotation * local_position;
            let velocity = event.rotation * local_velocity;

            let (min, max) = (effect.size_range[0], effect.size_range[1]);
            let scale = Vec3::splat(min + (max - min) * rng.next_f32());

            // Same as from bevy_pbr::utils
            // The lifetime needs to match in the shader in order to have the correct
            // animation length.
            fn rand(mut seed: u32) -> f32 {
                seed = seed.wrapping_mul(747796405).wrapping_add(2891336453);
                let word = ((seed >> ((seed >> 28) + 4)) ^ seed).wrapping_mul(277803737);
                return ((word >> 22) ^ word) as f32 * f32::from_bits(0x2f800004);
            }
            let seed = rng.next_u32() >> 16;
            let particle_lifetime =
                effect.lifetime[0] + (effect.lifetime[1] - effect.lifetime[0]) * rand(seed);

            // 8 bits reserved for light
            let mesh_tag = MeshTag(seed << 8);

            commands.spawn((
                Particle::new(
                    particle_lifetime,
                    effect.acceleration,
                    effect.collision,
                    effect.friction,
                ),
                Velocity(velocity),
                Transform {
                    translation,
                    scale,
                    ..default()
                },
                Mesh3d(mesh.clone()),
                MeshMaterial3d(material.clone()),
                mesh_tag,
                MovesWithOrigin,
            ));
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
        if particle.lifetime.is_finished() {
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
    let camera_transform = camera_transform.single().unwrap();

    for mut particle_transform in particles.iter_mut() {
        // TODO: This should be "-camera_transform.forward()", maybe I did the mesh the wrong way
        // around, or it's a problem that it's missing normals.
        particle_transform.look_to(camera_transform.forward(), camera_transform.up());
    }
}

#[derive(Component, Default, Deref, DerefMut)]
pub struct Velocity(pub Vec3);

trait AabbExt {
    fn new_particle(transform: &Transform) -> Aabb;

    fn intersection(&self, other: &Aabb) -> Option<Vec3A>;

    fn transform(&mut self, transform: &Transform);

    fn iter_block_positions(&self) -> impl IntoIterator<Item = IVec3>;
}

impl AabbExt for Aabb {
    fn new_particle(transform: &Transform) -> Self {
        Self {
            center: Vec3A::from(transform.translation),
            half_extents: Vec3A::from(transform.scale) * 0.5,
        }
    }

    fn intersection(&self, other: &Aabb) -> Option<Vec3A> {
        let distance = self.center - other.center;
        let overlap = self.half_extents + other.half_extents - distance.abs();

        if overlap.cmpgt(Vec3A::ZERO).all() {
            Some(overlap.copysign(distance))
        } else {
            None
        }
    }

    fn transform(&mut self, transform: &Transform) {
        let rot_mat = Mat3A::from_quat(transform.rotation);

        *self = Self {
            center: self.center + Vec3A::from(transform.translation),
            half_extents: rot_mat.abs() * self.half_extents * Vec3A::from(transform.scale),
        }
    }

    /// Iterator over the block positions inside the collider
    fn iter_block_positions(&self) -> impl IntoIterator<Item = IVec3> {
        let min = self.min().floor().as_ivec3();
        let max = self.max().floor().as_ivec3();
        (min.x..=max.x).flat_map(move |x| {
            (min.z..=max.z).flat_map(move |z| (min.y..=max.y).map(move |y| IVec3::new(x, y, z)))
        })
    }
}

// BUG: Wanted to use Vec3A end to end, but the Vec3A::max_element function considers NaN to be
// greater than any number, where Vec3::max_element is opposite.
fn simulate_particle_physics(
    origin: Res<Origin>,
    world_map: Res<WorldMap>,
    time: Res<Time>,
    blocks: Res<Blocks>,
    mut entities: Query<(&mut Transform, &mut Velocity, &Particle)>,
) {
    for (mut transform, mut velocity, particle) in entities.iter_mut() {
        velocity.0 += particle.acceleration * time.delta_secs();

        let delta_time = Vec3A::splat(time.delta_secs());
        let new_position = Vec3A::from(transform.translation + velocity.0 * time.delta_secs());

        let mut move_back = Vec3A::ZERO;
        for directional_velocity in [
            Vec3A::new(0.0, velocity.y, 0.0),
            Vec3A::new(velocity.x, 0.0, velocity.z),
        ] {
            if !particle.collision {
                continue;
            }

            let mut particle_aabb = Aabb::new_particle(&transform);
            particle_aabb.center += directional_velocity * time.delta_secs();

            for block_position in particle_aabb.iter_block_positions() {
                let world_block_position = origin.0 + block_position;

                let block_id = match world_map.get_block(&world_block_position) {
                    Some(id) => id,
                    None => continue,
                };
                let block_config = blocks.get_config(block_id);

                let rotation = world_map
                    .get_block_state(&world_block_position)
                    .map(BlockRotation::from)
                    .map(BlockRotation::as_quat)
                    .unwrap_or_default();

                let mut block_aabb = block_config.aabb().clone();
                block_aabb.transform(&Transform {
                    translation: block_position.as_vec3(),
                    rotation,
                    ..default()
                });

                let Some(overlap) = particle_aabb.intersection(&block_aabb) else {
                    continue;
                };

                if !block_config.is_solid() {
                    continue;
                }

                let backwards_time = overlap / -directional_velocity;
                // Small epsilon to delta time because of precision.
                let valid_axes = backwards_time.cmplt(delta_time + delta_time / 100.0)
                    & backwards_time.cmpgt(Vec3A::ZERO);
                let resolution_axis =
                    Vec3A::select(valid_axes, backwards_time, Vec3A::MIN).max_element();

                if resolution_axis == backwards_time.y {
                    move_back.y = overlap.y + overlap.y / 100.0;
                    velocity.y = 0.0;
                } else if resolution_axis == backwards_time.x {
                    move_back.x = overlap.x + overlap.x / 100.0;
                    velocity.x = 0.0;
                } else if resolution_axis == backwards_time.z {
                    move_back.z = overlap.z + overlap.z / 100.0;
                    velocity.z = 0.0;
                }
            }
        }

        transform.translation = (new_position + move_back).into();

        let mass = 1.0;
        velocity.0 *= Vec3::from((-particle.friction / mass * time.delta_secs()).exp());
    }
}
