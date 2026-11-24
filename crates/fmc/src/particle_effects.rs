use std::collections::HashMap;

use crate::bevy::{math::DQuat, prelude::*};
use fmc_protocol::messages;

use crate::{
    assets::AssetSet,
    networking::Server,
    transform::GlobalTransform,
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
                send_emitters,
                send_emitters_on_chunk_subscription,
                remove_emitters,
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
pub struct ParticleEmitter {
    /// Id of the particle effect to spawn
    pub effect: ParticleEffectId,
    /// Path to the texture, relative to "textures/"
    pub texture: String,
    /// Tint
    pub color: Vec4,
    /// Minimum and maximum time in seconds between each time the effect is spawned
    pub interval: [f32; 2],
}

impl ParticleEmitter {
    fn to_message(
        &self,
        entity: Entity,
        global_transform: &GlobalTransform,
    ) -> messages::ParticleEmitter {
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

        messages::ParticleEmitter {
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

fn send_emitters(
    net: Res<Server>,
    chunk_subscriptions: Res<ChunkSubscriptions>,
    emitter_query: Query<
        (
            Entity,
            &ParticleEmitter,
            &Visibility,
            &Observers,
            &GlobalTransform,
        ),
        Or<(Changed<ParticleEmitter>, Changed<Visibility>)>,
    >,
) {
    for (entity, emitter, visibility, observers, global_transform) in emitter_query.iter() {
        let chunk_position = ChunkPosition::from(global_transform.translation());
        let Some(subs) = chunk_subscriptions.get_subscribers(&chunk_position) else {
            continue;
        };

        if visibility.is_visible() {
            net.send_many(
                observers.filter(subs),
                emitter.to_message(entity, global_transform),
            );
        } else {
            net.send_many(
                observers.filter(subs),
                messages::DeleteParticleEmitter {
                    id: entity.index_u32(),
                },
            );
        }
    }
}

fn send_emitters_on_chunk_subscription(
    net: Res<Server>,
    entity_map: Res<EntityMap>,
    emitter_query: Query<(&ParticleEmitter, &Visibility, &Observers, &GlobalTransform)>,
    mut chunk_sub_events: MessageReader<ChunkSubscriptionEvent>,
) {
    for chunk_sub in chunk_sub_events.read() {
        for entity in entity_map.iter_entities(&chunk_sub.chunk_position) {
            let Ok((emitter, visibility, observers, global_transform)) = emitter_query.get(entity)
            else {
                continue;
            };

            if !visibility.is_visible() || !observers.is_included(chunk_sub.player_entity) {
                continue;
            }

            net.send_one(
                chunk_sub.player_entity,
                emitter.to_message(entity, global_transform),
            );
        }
    }
}

fn remove_emitters(
    net: Res<Server>,
    entity_map: Res<EntityMap>,
    chunk_subscriptions: Res<ChunkSubscriptions>,
    mut removed_emitters: RemovedComponents<ParticleEmitter>,
) {
    for entity in removed_emitters.read() {
        let Some(chunk_position) = entity_map.chunk_position(&entity) else {
            continue;
        };

        if let Some(subs) = chunk_subscriptions.get_subscribers(&chunk_position) {
            net.send_many(
                subs,
                messages::DeleteParticleEmitter {
                    id: entity.index_u32(),
                },
            );
        }
    }
}
