use std::{collections::HashMap, time::Duration};

use bevy::{
    camera::primitives::Aabb,
    math::{DVec3, Vec3A},
    mesh::MeshTag,
    prelude::*,
};
use fmc_protocol::messages;
use smallvec::SmallVec;

use crate::{
    assets::particles::{ParticleEffect, ParticleEffectAssetId, ParticleEffects, ParticleTextures},
    blocks::{BlockRotation, BlockState, Blocks, ParticleEmitterConfig},
    game_state::GameState,
    rendering::{light_texture::LIGHT_TEXTURE, materials::ParticleMaterial},
    settings::Settings,
    utils,
    world::{
        BlockPosition, Chunk, ChunkPosition, MovesWithOrigin, NewChunkEvent, Origin, WorldMap,
    },
};

const EMITTER_DISTANCE: i32 = 2;

pub struct ParticlePlugin;
impl Plugin for ParticlePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BlockParticleEmitters>()
            .init_resource::<ServerParticleEmitters>()
            .add_systems(OnEnter(GameState::Launcher), clear_emitters)
            .add_systems(
                FixedUpdate,
                simulate_particle_physics.run_if(in_state(GameState::Playing)),
            )
            .add_systems(
                Update,
                (
                    handle_particles_from_server,
                    (
                        (load_block_emitters, unload_block_emitters).chain(),
                        (handle_server_emitters, unload_server_emitters).chain(),
                        emit_from_emitters,
                    )
                        .chain(),
                    despawn_particles,
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

        spawn_effect(
            &mut commands,
            &asset_server,
            &mut rng,
            effect,
            texture,
            event.color,
            time.elapsed_secs_wrapped(),
            origin.as_ivec3(),
            origin.to_local(event.position),
            event.rotation,
        );
    }
}

fn spawn_effect(
    commands: &mut Commands,
    asset_server: &AssetServer,
    rng: &mut utils::Rng,
    effect: &ParticleEffect,
    texture: Handle<Image>,
    color: Vec4,
    spawn_time: f32,
    origin: IVec3,
    position: Vec3,
    rotation: Quat,
) {
    let material = asset_server.add(ParticleMaterial {
        texture,
        base_color: Srgba::from_vec4(color),
        lifetime: Vec2::from_array(effect.lifetime),
        random_uv: effect.random_uv.unwrap_or_default(),
        spawn_time,
        light_texture: LIGHT_TEXTURE,
        origin,
    });

    let mesh = asset_server.add(Rectangle::from_length(0.5).mesh().build());

    for _ in 0..effect.count {
        let local_position = effect.position(rng);
        let local_velocity = effect.velocity(rng, local_position);
        let translation = position + rotation * local_position;
        let velocity = rotation * local_velocity;

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

        let mesh_tag = MeshTag(seed);

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

#[derive(Resource, Default)]
struct BlockParticleEmitters {
    chunks: HashMap<ChunkPosition, HashMap<usize, SmallVec<[ParticleEmitter; 2]>>>,
}

#[derive(Resource, Default, Deref, DerefMut)]
struct ServerParticleEmitters(HashMap<u32, ParticleEmitter>);

struct ParticleEmitter {
    effect: ParticleEffectAssetId,
    position: DVec3,
    rotation: Quat,
    texture: Handle<Image>,
    color: Vec4,
    interval: [f32; 2],
    timer: f32,
}

impl ParticleEmitter {
    fn from_block(
        block_position: BlockPosition,
        state: Option<BlockState>,
        config: &ParticleEmitterConfig,
        rng: &mut utils::Rng,
    ) -> Self {
        let rotation = state
            .map(|state| state.rotation().as_quat())
            .unwrap_or_default();
        let bottom_center = Vec3::new(0.5, 0.0, 0.5);
        let offset = bottom_center + rotation * (config.offset - bottom_center);

        Self {
            effect: config.effect,
            position: block_position.as_dvec3() + offset.as_dvec3(),
            rotation,
            texture: config.texture.clone(),
            color: config.color,
            interval: config.interval,
            timer: rng.next_f32() * config.interval[1],
        }
    }
}

fn clear_emitters(
    mut block_emitters: ResMut<BlockParticleEmitters>,
    mut server_emitters: ResMut<ServerParticleEmitters>,
) {
    block_emitters.chunks.clear();
    server_emitters.clear();
}

fn load_block_emitters(
    blocks: Res<Blocks>,
    world_map: Res<WorldMap>,
    mut emitters: ResMut<BlockParticleEmitters>,
    mut new_chunks: MessageReader<NewChunkEvent>,
    mut block_updates: MessageReader<messages::BlockUpdates>,
    mut rng: Local<utils::Rng>,
) {
    for event in new_chunks.read() {
        emitters.chunks.remove(&event.position);

        let Some(chunk) = world_map.get_chunk(&event.position) else {
            continue;
        };

        if chunk.is_uniform() {
            continue;
        }

        let mut chunk_emitters = HashMap::new();
        for (index, block_id) in chunk.iter_blocks() {
            let configs = blocks.get_config(*block_id).particle_emitters();
            if configs.is_empty() {
                continue;
            }

            let position = BlockPosition::from(index);
            let state = chunk.get_block_state(
                position.x as usize,
                position.y as usize,
                position.z as usize,
            );
            let block_position = BlockPosition::from(event.position) + position;
            chunk_emitters.insert(
                index,
                configs
                    .iter()
                    .map(|config| {
                        ParticleEmitter::from_block(block_position, state, config, &mut rng)
                    })
                    .collect(),
            );
        }

        if !chunk_emitters.is_empty() {
            emitters.chunks.insert(event.position, chunk_emitters);
        }
    }

    for event in block_updates.read() {
        let chunk_position = ChunkPosition::from(event.chunk_position);
        if !world_map.contains_chunk(&chunk_position) {
            continue;
        }

        for (index, block_id, state) in event.blocks.iter() {
            if let Some(chunk_emitters) = emitters.chunks.get_mut(&chunk_position) {
                chunk_emitters.remove(index);
                if chunk_emitters.is_empty() {
                    emitters.chunks.remove(&chunk_position);
                }
            }

            if !blocks.contains(*block_id) {
                continue;
            }

            let configs = blocks.get_config(*block_id).particle_emitters();
            if configs.is_empty() {
                continue;
            }

            let block_position = BlockPosition::from(chunk_position) + BlockPosition::from(*index);
            emitters.chunks.entry(chunk_position).or_default().insert(
                *index,
                configs
                    .iter()
                    .map(|config| {
                        ParticleEmitter::from_block(
                            block_position,
                            state.map(BlockState),
                            config,
                            &mut rng,
                        )
                    })
                    .collect(),
            );
        }
    }
}

fn unload_block_emitters(world_map: Res<WorldMap>, mut emitters: ResMut<BlockParticleEmitters>) {
    if emitters.chunks.is_empty() {
        return;
    }

    emitters
        .chunks
        .retain(|position, _| world_map.contains_chunk(position));
}

fn emit_from_emitters(
    mut commands: Commands,
    time: Res<Time>,
    origin: Res<Origin>,
    asset_server: Res<AssetServer>,
    particle_effects: Res<ParticleEffects>,
    mut block_emitters: ResMut<BlockParticleEmitters>,
    mut server_emitters: ResMut<ServerParticleEmitters>,
    mut rng: Local<utils::Rng>,
) {
    let in_range = |chunk_position: ChunkPosition| {
        let distance = (chunk_position - origin.0).abs() / IVec3::splat(Chunk::SIZE as i32);
        distance.max_element() <= EMITTER_DISTANCE
    };

    let block_emitters = block_emitters
        .chunks
        .iter_mut()
        .filter(|(chunk_position, _)| in_range(**chunk_position))
        .flat_map(|(_, chunk_emitters)| chunk_emitters.values_mut().flatten());

    let server_emitters = server_emitters
        .values_mut()
        .filter(|emitter| in_range(ChunkPosition::from(emitter.position)));

    for emitter in block_emitters.chain(server_emitters) {
        emitter.timer -= time.delta_secs();
        if emitter.timer > 0.0 {
            continue;
        }

        let [min, max] = emitter.interval;
        emitter.timer = min + (max - min) * rng.next_f32();

        let Some(effect) = particle_effects.get(&emitter.effect) else {
            continue;
        };

        spawn_effect(
            &mut commands,
            &asset_server,
            &mut rng,
            effect,
            emitter.texture.clone(),
            emitter.color,
            time.elapsed_secs_wrapped(),
            origin.as_ivec3(),
            origin.to_local(emitter.position),
            emitter.rotation,
        );
    }
}

fn handle_server_emitters(
    particle_effects: Res<ParticleEffects>,
    particle_textures: Res<ParticleTextures>,
    mut emitters: ResMut<ServerParticleEmitters>,
    mut deleted_emitters: MessageReader<messages::DeleteParticleEmitter>,
    mut new_emitters: MessageReader<messages::ParticleEmitter>,
    mut rng: Local<utils::Rng>,
) {
    for deleted_emitter in deleted_emitters.read() {
        emitters.remove(&deleted_emitter.id);
    }

    for new_emitter in new_emitters.read() {
        if particle_effects.get(&new_emitter.effect).is_none() {
            warn!(
                "Received invalid particle effect id: {}",
                new_emitter.effect
            );
            continue;
        }

        let Some(texture) = particle_textures.get(&new_emitter.texture).cloned() else {
            warn!(
                "Received invalid particle texture, no texture at: {}",
                new_emitter.texture
            );
            continue;
        };

        emitters.insert(
            new_emitter.id,
            ParticleEmitter {
                effect: new_emitter.effect,
                position: new_emitter.position,
                rotation: new_emitter.rotation,
                texture,
                color: new_emitter.color,
                interval: new_emitter.interval,
                timer: rng.next_f32() * new_emitter.interval[1],
            },
        );
    }
}

fn unload_server_emitters(
    origin: Res<Origin>,
    settings: Res<Settings>,
    mut emitters: ResMut<ServerParticleEmitters>,
) {
    if !origin.is_changed() && !settings.is_changed() {
        return;
    }

    emitters.retain(|_, emitter| {
        let chunk_offset = ChunkPosition::from(emitter.position) - origin.0;
        let distance = chunk_offset.abs() / IVec3::splat(Chunk::SIZE as i32);
        distance.max_element() <= settings.render_distance as i32
    });
}

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

#[derive(Component, Default, Deref, DerefMut)]
struct Velocity(pub Vec3);

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
                let world_block_position = BlockPosition(origin.as_ivec3() + block_position);

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
