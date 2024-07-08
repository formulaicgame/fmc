use std::collections::{HashMap, HashSet};

use bevy::{math::DVec3, prelude::*};
use fmc_networking::{messages, ConnectionId, NetworkServer};
use indexmap::IndexMap;

use crate::{
    bevy_extensions::f64_transform::{GlobalTransform, Transform},
    database::Database,
    physics::{shapes::Aabb, PhysicsSystems, Velocity},
    utils,
    world::chunk_manager::{ChunkSubscriptionEvent, ChunkSubscriptions},
};

// TODO:
//use super::world_map::chunk_manager::ChunkUnloadEvent;

pub const MODEL_PATH: &str = "./resources/client/textures/models/";

// Type used to identify the asset of a model.
pub type ModelId = u32;

pub struct ModelPlugin;
impl Plugin for ModelPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ModelMap::default())
            .add_systems(PreStartup, load_models)
            .add_systems(
                Update,
                (
                    send_models_on_chunk_subscription,
                    update_model_transform,
                    update_model_assets,
                    update_visibility,
                    play_move_animation
                        .before(send_animations)
                        .after(PhysicsSystems),
                    send_animations,
                ),
            )
            // TODO: Maybe all of these systems should be PostUpdate. This way Update is the do
            // things place, and PostUpdate is the send to client place.
            //
            // XXX: PostUpdate because RemovedComponents is only available from the stage it was
            // removed up to CoreStage::Last.
            .add_systems(PostUpdate, remove_models);
    }
}

// TODO: We only need the json part of the gltf files, loading the binary parts is wasteful and it
// doesn't reuse the memory. This is also the only use of the gltf crate. I read it to json before,
// but I think I experienced a problem computing the aabb's because there were accessors with
// min/max values that didn't relate to meshes. Need to investigate if this is true (I might have
// messed up some pivot points in Blockbench)
fn load_models(mut commands: Commands, database: Res<Database>) {
    let directory = std::fs::read_dir(MODEL_PATH).expect(&format!(
        "Could not read files from model directory, make sure it is present at '{}'.",
        &MODEL_PATH
    ));

    // Instead of going 'load models from db -> read the files we need from directory' we go 'read
    // all files to configurations -> match to loaded models'. This is because only the file stem
    // is stored .e.g. model_name.gltf -> model_name, which makes it hard to reconstruct as a path
    // since the extension can be any of gltf/glb/json.
    // As a side effect, this allows changing the file type after the model has been registered as
    // part of a world.
    let mut configs = HashMap::with_capacity(directory.size_hint().0);

    for dir_entry in directory {
        let path = match dir_entry {
            Ok(d) => d.path(),
            Err(e) => panic!("Failed to read the filename of a model, Error: {}", e),
        };

        let Some(extension) = path.extension() else {
            panic!(
                "Invalid model file at '{}', the file is missing a file extension.",
                path.display()
            )
        };

        let mut config = ModelConfig {
            // We change the id to the correct one when moving the configs into Models.
            id: 0,
            animations: HashMap::new(),
            aabb: Aabb::default(),
        };

        if extension == "json" {
            // Block models can be defined through json files.
            config.aabb =
                Aabb::from_min_max(DVec3::new(-0.5, 0.0, -0.5), DVec3::new(0.5, 1.0, 0.5));
        } else if extension == "glb" || extension == "gltf" {
            let gltf = match gltf::Gltf::open(&path) {
                Ok(m) => m,
                Err(e) => panic!(
                    "Failed to open gltf model at: {}\nError: {}",
                    path.display(),
                    e
                ),
            };

            let mut min = Vec3::MAX;
            let mut max = Vec3::MIN;

            for node in gltf.nodes() {
                let Some(mesh) = node.mesh() else { continue };

                let translation = Vec3::from_array(node.transform().decomposed().0);

                for primitive in mesh.primitives() {
                    let bounds = primitive.bounding_box();
                    min = min.min(Vec3::from_array(bounds.min) + translation);
                    max = max.max(Vec3::from_array(bounds.max) + translation);
                }
            }

            config.aabb = Aabb::from_min_max(min.as_dvec3(), max.as_dvec3());

            for animation in gltf.document.animations() {
                if let Some(name) = animation.name() {
                    config
                        .animations
                        .insert(name.to_string(), animation.index() as u32);
                }
            }
        } else {
            panic!("Invalid model");
        }

        // TODO: These unwraps can probably fail
        let name = path.file_stem().unwrap().to_str().unwrap().to_lowercase();

        configs.insert(name, config);
    }

    let model_names = database.load_models();

    let mut model_configs = Models(IndexMap::with_capacity(model_names.len()));

    for model_name in model_names {
        let Some(mut config) = configs.remove(&model_name) else {
            panic!(
                "Missing model '{}', make sure it exists at '{}' as a gltf/glb/json file",
                model_name, MODEL_PATH
            );
        };

        config.id = model_configs.0.len() as u32;

        model_configs.0.insert(model_name, config);
    }

    commands.insert_resource(model_configs);
}

// TODO: Setting the default move animation is almost always something you want to do, but only on
// initial spawn. Maybe introduce a transient component in this bundle that can be removed when
// added.
#[derive(Bundle)]
pub struct ModelBundle {
    pub model: Model,
    pub animations: ModelAnimations,
    pub visibility: ModelVisibility,
    pub global_transform: GlobalTransform,
    pub transform: Transform,
}

#[derive(Component)]
pub struct Model {
    pub id: ModelId,
}

#[derive(Component)]
pub struct ModelVisibility {
    pub is_visible: bool,
}

impl Default for ModelVisibility {
    fn default() -> Self {
        Self { is_visible: true }
    }
}

enum Animation {
    Play(u32),
    StopRepeating(u32),
    PlayRepeating(u32),
}

#[derive(Component, Default)]
pub struct ModelAnimations {
    move_animation: Option<u32>,
    playing_move_animation: bool,
    repeating: HashSet<u32>,
    animation_queue: Vec<Animation>,
}

impl ModelAnimations {
    pub fn play(&mut self, animation_index: u32) {
        self.animation_queue.push(Animation::Play(animation_index));
    }

    pub fn play_repeating(&mut self, animation_index: u32) {
        self.animation_queue
            .push(Animation::PlayRepeating(animation_index));
    }

    pub fn stop(&mut self, animation_index: u32) {
        self.animation_queue
            .push(Animation::StopRepeating(animation_index));
    }

    pub fn play_on_move(&mut self, animation_index: Option<u32>) {
        if let Some(prev) = self.move_animation.take() {
            self.animation_queue.push(Animation::StopRepeating(prev));
        }

        self.move_animation = animation_index;
    }
}

pub struct ModelConfig {
    pub id: ModelId,
    // Map from animation name (as stored in the gltf file) to its index
    pub animations: HashMap<String, u32>,
    pub aabb: Aabb,
}

// The models are stored as an IndexMap where the index corresponds to the model's asset id.
#[derive(Resource)]
pub struct Models(IndexMap<String, ModelConfig>);

impl Models {
    pub fn get_by_name(&self, name: &str) -> &ModelConfig {
        &self.0[name]
    }

    pub fn get_by_id(&self, id: ModelId) -> &ModelConfig {
        &self.0[id as usize]
    }

    pub fn asset_ids(&self) -> HashMap<String, ModelId> {
        return self
            .0
            .keys()
            .cloned()
            .enumerate()
            .map(|(id, name)| (name, id as ModelId))
            .collect();
    }
}

/// Keeps track of which chunk every entity with a model is currently in.
#[derive(Default, Resource)]
pub struct ModelMap {
    position2entity: HashMap<IVec3, HashSet<Entity>>,
    entity2position: HashMap<Entity, IVec3>,
}

impl ModelMap {
    pub fn get_entities(&self, chunk_position: &IVec3) -> Option<&HashSet<Entity>> {
        return self.position2entity.get(chunk_position);
    }

    fn insert_or_move(&mut self, chunk_position: IVec3, entity: Entity) {
        if let Some(current_chunk_pos) = self.entity2position.get(&entity) {
            // Move model from one chunk to another
            if current_chunk_pos == &chunk_position {
                return;
            } else {
                let past_chunk_pos = self.entity2position.remove(&entity).unwrap();

                self.position2entity
                    .get_mut(&past_chunk_pos)
                    .unwrap()
                    .remove(&entity);

                self.position2entity
                    .entry(chunk_position)
                    .or_insert(HashSet::new())
                    .insert(entity);

                self.entity2position.insert(entity, chunk_position);
            }
        } else {
            // First time seeing model, insert it normally
            self.position2entity
                .entry(chunk_position)
                .or_insert(HashSet::new())
                .insert(entity);
            self.entity2position.insert(entity, chunk_position);
        }
    }
}

fn remove_models(
    net: Res<NetworkServer>,
    mut model_map: ResMut<ModelMap>,
    chunk_subscriptions: Res<ChunkSubscriptions>,
    mut deleted_models: RemovedComponents<Model>,
) {
    for entity in deleted_models.read() {
        let chunk_pos = if let Some(position) = model_map.entity2position.remove(&entity) {
            model_map
                .position2entity
                .get_mut(&position)
                .unwrap()
                .remove(&entity);
            position
        } else {
            // TODO: This if condition can be removed, I just want to test for a while that I didn't
            // mess up.
            panic!("All models that are spawned should be entered into the model map. \
                   If when trying to delete a model it doesn't exist in the model map that is big bad.")
        };

        if let Some(subs) = chunk_subscriptions.get_subscribers(&chunk_pos) {
            net.send_many(subs, messages::DeleteModel { id: entity.index() });
        }
    }
}

// TODO: Split position, rotation and scale into packets?
fn update_model_transform(
    net: Res<NetworkServer>,
    chunk_subscriptions: Res<ChunkSubscriptions>,
    mut model_map: ResMut<ModelMap>,
    model_query: Query<
        (Entity, &GlobalTransform, &ModelVisibility, Ref<Model>),
        Changed<GlobalTransform>,
    >,
) {
    for (entity, global_transform, visibility, tracker) in model_query.iter() {
        let transform = global_transform.compute_transform();
        let chunk_position =
            utils::world_position_to_chunk_position(transform.translation.as_ivec3());

        model_map.insert_or_move(chunk_position, entity);

        if !visibility.is_visible || tracker.is_added() {
            continue;
        }

        let subs = match chunk_subscriptions.get_subscribers(&chunk_position) {
            Some(subs) => subs,
            None => continue,
        };

        net.send_many(
            subs,
            messages::ModelUpdateTransform {
                id: entity.index(),
                position: transform.translation,
                rotation: transform.rotation.as_quat(),
                scale: transform.scale.as_vec3(),
            },
        );
    }
}

// TODO: Requiring models to have a Velocity seems unfortunate. Maybe have a separate component to
// keep track of the velocity through difference in changes to the transform, with some lower and
// higher bound for stopping/starting the animation.
fn play_move_animation(
    mut moved_models: Query<(&mut ModelAnimations, &Velocity), Changed<GlobalTransform>>,
) {
    for (mut animations, velocity) in moved_models.iter_mut() {
        let Some(move_animation) = animations.move_animation else {
            continue;
        };

        if !animations.playing_move_animation && velocity.is_moving() {
            animations.playing_move_animation = true;
            animations.play_repeating(move_animation);
        } else if animations.playing_move_animation && !velocity.is_moving() {
            animations.playing_move_animation = false;
            animations.stop(move_animation);
        }
    }
}

// TODO: I'm not entirely sure what the purpose of this was. Why not just replace the model?
// Remember to make model.id private if changed.
fn update_model_assets(
    net: Res<NetworkServer>,
    chunk_subscriptions: Res<ChunkSubscriptions>,
    model_query: Query<(Entity, Ref<Model>, &Transform, &ModelVisibility), Changed<Model>>,
) {
    for (entity, model, transform, visibility) in model_query.iter() {
        if !visibility.is_visible || model.is_added() {
            continue;
        }

        let chunk_pos = utils::world_position_to_chunk_position(transform.translation.as_ivec3());

        let subs = match chunk_subscriptions.get_subscribers(&chunk_pos) {
            Some(subs) => subs,
            None => continue,
        };

        net.send_many(
            subs,
            messages::ModelUpdateAsset {
                id: entity.index(),
                asset: model.id,
            },
        );
    }
}

// TODO: Animations must be sent
fn update_visibility(
    net: Res<NetworkServer>,
    chunk_subscriptions: Res<ChunkSubscriptions>,
    model_query: Query<
        (Entity, &Model, &ModelVisibility, &GlobalTransform),
        Or<(Changed<ModelVisibility>, Added<Model>)>,
    >,
) {
    for (entity, model, visibility, transform) in model_query.iter() {
        let transform = transform.compute_transform();

        let chunk_pos = utils::world_position_to_chunk_position(transform.translation.as_ivec3());

        let subs = match chunk_subscriptions.get_subscribers(&chunk_pos) {
            Some(subs) => subs,
            None => continue,
        };

        if visibility.is_visible {
            net.send_many(
                subs,
                messages::NewModel {
                    parent_id: None,
                    id: entity.index(),
                    asset: model.id,
                    position: transform.translation,
                    rotation: transform.rotation.as_quat(),
                    scale: transform.scale.as_vec3(),
                },
            );
        } else {
            net.send_many(subs, messages::DeleteModel { id: entity.index() });
        }
    }
}

fn send_models_on_chunk_subscription(
    net: Res<NetworkServer>,
    model_map: Res<ModelMap>,
    player_query: Query<&ConnectionId>,
    model_query: Query<(
        Option<&Parent>,
        &Model,
        &ModelAnimations,
        &GlobalTransform,
        &ModelVisibility,
    )>,
    mut chunk_sub_events: EventReader<ChunkSubscriptionEvent>,
) {
    for chunk_sub in chunk_sub_events.read() {
        if let Some(model_entities) = model_map.get_entities(&chunk_sub.chunk_position) {
            for entity in model_entities.iter() {
                let Ok((maybe_player_parent, model, animations, transform, visibility)) =
                    model_query.get(*entity)
                else {
                    continue;
                };

                if !visibility.is_visible {
                    continue;
                }

                // Don't send the player models to the players they belong to.
                if let Some(parent) = maybe_player_parent {
                    let connection_id = player_query.get(parent.get()).unwrap();
                    if connection_id == &chunk_sub.connection_id {
                        continue;
                    }
                }

                let transform = transform.compute_transform();

                net.send_one(
                    chunk_sub.connection_id,
                    messages::NewModel {
                        id: entity.index(),
                        parent_id: None,
                        position: transform.translation,
                        rotation: transform.rotation.as_quat(),
                        scale: transform.scale.as_vec3(),
                        asset: model.id,
                    },
                );

                if animations.playing_move_animation {
                    let animation_index = animations.move_animation.unwrap();
                    net.send_one(
                        chunk_sub.connection_id,
                        messages::ModelPlayAnimation {
                            model_id: entity.index(),
                            animation_index,
                            repeat: true,
                        },
                    );
                }

                for animation_index in animations.repeating.iter().copied() {
                    net.send_one(
                        chunk_sub.connection_id,
                        messages::ModelPlayAnimation {
                            model_id: entity.index(),
                            animation_index,
                            repeat: true,
                        },
                    );
                }
            }
        }
    }
}

fn send_animations(
    net: Res<NetworkServer>,
    chunk_subscriptions: Res<ChunkSubscriptions>,
    mut animation_query: Query<
        (Entity, &mut ModelAnimations, &GlobalTransform),
        Changed<ModelAnimations>,
    >,
) {
    for (entity, mut model_animations, transform) in animation_query.iter_mut() {
        let chunk_position =
            utils::world_position_to_chunk_position(transform.translation().floor().as_ivec3());

        let Some(subs) = chunk_subscriptions.get_subscribers(&chunk_position) else {
            model_animations.animation_queue.clear();
            continue;
        };

        for animation in model_animations.animation_queue.drain(..) {
            match animation {
                Animation::Play(animation_index) => net.send_many(
                    subs,
                    messages::ModelPlayAnimation {
                        model_id: entity.index(),
                        animation_index,
                        repeat: false,
                    },
                ),
                Animation::PlayRepeating(animation_index) => net.send_many(
                    subs,
                    messages::ModelPlayAnimation {
                        model_id: entity.index(),
                        animation_index,
                        repeat: true,
                    },
                ),
                Animation::StopRepeating(animation_index) => net.send_many(
                    subs,
                    messages::ModelPlayAnimation {
                        model_id: entity.index(),
                        animation_index,
                        repeat: false,
                    },
                ),
            }
        }
    }
}
