use std::collections::HashMap;

use bevy::{math::DQuat, prelude::*};
use fmc_protocol::messages;

use crate::{
    assets::AssetSet,
    bevy_extensions::f64_transform::GlobalTransform,
    networking::Server,
    visibility::{EntityMap, Observers, Visibility, VisibilitySystems},
    world::{ChunkSubscriptionEvent, ChunkSubscriptions, chunk::ChunkPosition},
};

pub const PARTICLE_EFFECT_PATH: &str = "./assets/client/particle_effects/";

pub type ParticleEffectId = u32;

pub struct ParticleEffectPlugin;
impl Plugin for ParticleEffectPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            PreStartup,
            load_particle_effects.in_set(AssetSet::ParticleEffects),
        )
        .add_systems(
            PostUpdate,
            (
                send_spawners,
                send_spawners_on_chunk_subscription,
                remove_spawners,
            )
                .after(VisibilitySystems),
        );
    }
}

fn load_particle_effects(mut commands: Commands) {
    let directory = std::fs::read_dir(PARTICLE_EFFECT_PATH).expect(&format!(
        "Could not read files from particle effect directory, make sure it is present at '{}'.",
        PARTICLE_EFFECT_PATH
    ));

    let mut effects = ParticleEffects {
        ids: HashMap::new(),
    };

    for (id, dir_entry) in directory.enumerate() {
        let path = match dir_entry {
            Ok(d) => d.path(),
            Err(e) => panic!(
                "Failed to read the filename of a particle effect, Error: {}",
                e
            ),
        };

        let name = path.file_stem().unwrap().to_str().unwrap().to_owned();
        effects.ids.insert(name, id as ParticleEffectId);
    }

    commands.insert_resource(effects);
}

/// Maps each particle effect's filename to a unique id.
#[derive(Resource, Default)]
pub struct ParticleEffects {
    ids: HashMap<String, ParticleEffectId>,
}

impl ParticleEffects {
    pub fn get_id(&self, name: &str) -> Option<ParticleEffectId> {
        self.ids.get(name).cloned()
    }

    pub fn ids(&self) -> &HashMap<String, ParticleEffectId> {
        &self.ids
    }
}

/// Repeatedly spawns a particle effect at the position of the entity.
#[derive(Component, Clone, Debug)]
#[require(Visibility, Observers)]
pub struct ParticleSpawner {
    /// Id of the particle effect to spawn
    pub effect: ParticleEffectId,
    /// Path to the texture, relative to "textures/"
    pub texture: String,
    /// Tint
    pub color: Vec4,
    /// Minimum and maximum time in seconds between each time the effect is spawned
    pub interval: [f32; 2],
}

impl ParticleSpawner {
    fn to_message(
        &self,
        entity: Entity,
        global_transform: &GlobalTransform,
    ) -> messages::ParticleSpawner {
        // XXX: Only the rotation around the y axis is sent. This might not be completely correct,
        // but it serves my purposes of rotating block models, change if needed.
        let mut rotation = global_transform.rotation();
        rotation.x = 0.0;
        rotation.z = 0.0;
        let rotation = if rotation.length_squared() > 0.0 {
            rotation.normalize()
        } else {
            DQuat::IDENTITY
        };

        messages::ParticleSpawner {
            id: entity.index_u32(),
            position: global_transform.translation(),
            rotation: rotation.as_quat(),
            effect: self.effect,
            texture: self.texture.clone(),
            color: self.color,
            interval: self.interval,
        }
    }
}

fn send_spawners(
    net: Res<Server>,
    chunk_subscriptions: Res<ChunkSubscriptions>,
    spawner_query: Query<
        (
            Entity,
            &ParticleSpawner,
            &Visibility,
            &Observers,
            &GlobalTransform,
        ),
        Or<(Changed<ParticleSpawner>, Changed<Visibility>)>,
    >,
) {
    for (entity, spawner, visibility, observers, global_transform) in spawner_query.iter() {
        let chunk_position = ChunkPosition::from(global_transform.translation());
        let Some(subs) = chunk_subscriptions.get_subscribers(&chunk_position) else {
            continue;
        };

        if visibility.is_visible() {
            net.send_many(
                observers.filter(subs),
                spawner.to_message(entity, global_transform),
            );
        } else {
            net.send_many(
                observers.filter(subs),
                messages::DeleteParticleSpawner {
                    id: entity.index_u32(),
                },
            );
        }
    }
}

fn send_spawners_on_chunk_subscription(
    net: Res<Server>,
    entity_map: Res<EntityMap>,
    spawner_query: Query<(&ParticleSpawner, &Visibility, &Observers, &GlobalTransform)>,
    mut chunk_sub_events: MessageReader<ChunkSubscriptionEvent>,
) {
    for chunk_sub in chunk_sub_events.read() {
        for entity in entity_map.iter_entities(&chunk_sub.chunk_position) {
            let Ok((spawner, visibility, observers, global_transform)) = spawner_query.get(entity)
            else {
                continue;
            };

            if !visibility.is_visible() || !observers.is_included(chunk_sub.player_entity) {
                continue;
            }

            net.send_one(
                chunk_sub.player_entity,
                spawner.to_message(entity, global_transform),
            );
        }
    }
}

fn remove_spawners(
    net: Res<Server>,
    entity_map: Res<EntityMap>,
    chunk_subscriptions: Res<ChunkSubscriptions>,
    mut removed_spawners: RemovedComponents<ParticleSpawner>,
) {
    for entity in removed_spawners.read() {
        let Some(chunk_position) = entity_map.chunk_position(&entity) else {
            continue;
        };

        if let Some(subs) = chunk_subscriptions.get_subscribers(&chunk_position) {
            net.send_many(
                subs,
                messages::DeleteParticleSpawner {
                    id: entity.index_u32(),
                },
            );
        }
    }
}
