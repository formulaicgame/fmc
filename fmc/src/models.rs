use std::collections::{HashMap, HashSet};

use bevy::{math::DVec3, prelude::*};
use fmc_protocol::messages;
use indexmap::IndexMap;

use crate::{
    assets::AssetSet,
    bevy_extensions::f64_transform::{GlobalTransform, Transform, TransformSystem},
    database::Database,
    networking::Server,
    physics::{shapes::Aabb, Collider},
    players::Player,
    world::{chunk::ChunkPosition, ChunkSubscriptionEvent, ChunkSubscriptions},
};

// TODO use super::world_map::chunk_manager::ChunkUnloadEvent;

pub const MODEL_PATH: &str = "./assets/client/textures/models/";

// Used to identify the asset of a model.
pub type ModelId = u32;

pub struct ModelPlugin;
impl Plugin for ModelPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ModelMap::default())
            .add_systems(PreStartup, load_models.in_set(AssetSet::Models))
            .add_systems(
                PostUpdate,
                (
                    send_models_on_chunk_subscription.before(send_animations),
                    //update_model_assets,
                    apply_animations
                        .before(send_animations)
                        .after(TransformSystem::TransformPropagate),
                    send_animations,
                    remove_models,
                    send_model_transform,
                    update_visibility.after(TransformSystem::TransformPropagate),
                ),
            );
    }
}

// TODO: We only need the json part of the gltf files, loading the binary parts is wasteful and it
// doesn't reuse the memory. This is also the only use of the gltf crate. I read it to json before,
// but I think I experienced a problem computing the aabb's because there were accessors with
// min/max values that didn't relate to meshes. Need to investigate if this is true (I might have
// messed up some pivot points in Blockbench)
pub(crate) fn load_models(mut commands: Commands, database: Res<Database>) {
    let directory = std::fs::read_dir(MODEL_PATH).expect(&format!(
        "Could not read files from model directory, make sure it is present at '{}'.",
        &MODEL_PATH
    ));

    // Instead of going 'load models from db -> read the files we need from directory' we go 'read
    // all files to configurations -> match to db models'. This is because only the file stem
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
// TODO: With "custom" this is almost 200 bytes per model
#[derive(Component)]
#[require(ModelVisibility, AnimationPlayer, Transform)]
pub enum Model {
    Asset(ModelId),
    Custom {
        /// Mesh Indices
        mesh_indices: Vec<u32>,
        /// Mesh vertices
        mesh_vertices: Vec<[f32; 3]>,
        /// Mesh normals
        mesh_normals: Vec<[f32; 3]>,
        /// Texture uvs
        mesh_uvs: Option<Vec<[f32; 2]>>,
        /// Base color, hex encoded srgb
        material_base_color: String,
        /// Color texture of the mesh, pre-light color is material_base_color * this texture
        material_color_texture: Option<String>,
        /// Texture used for parallax mapping
        material_parallax_texture: Option<String>,
        /// Alpha blend mode, 0 = Opaque, 1 = mask, 2 = blend
        material_alpha_mode: u8,
        /// Alpha channel cutoff if the blend mode is Mask
        material_alpha_cutoff: f32,
        /// Render mesh from both sides
        material_double_sided: bool,
        /// Collider
        collider: Option<Collider>,
    },
}

#[derive(Component, Default)]
pub enum ModelVisibility {
    Hidden,
    #[default]
    Visible,
}

impl ModelVisibility {
    pub fn is_visible(&self) -> bool {
        matches!(self, Self::Visible)
    }
}

pub struct Animation {
    restart: bool,
    animation_index: u32,
    repeat: bool,
    transition_from: Option<u32>,
    transition_duration: f32,
}

impl Animation {
    pub fn repeat(&mut self) -> &mut Self {
        self.repeat = true;
        self
    }

    pub fn restart(&mut self) -> &mut Self {
        self.restart = true;
        self
    }

    pub fn transition(&mut self, from: u32, duration: f32) -> &mut Self {
        self.transition_from = Some(from);
        self.transition_duration = duration;
        self
    }
}

#[derive(Component, Default)]
pub struct AnimationPlayer {
    // The entity of the model being animated, if None, defaults to the entity the animation player
    // is part of.
    target: Option<Entity>,
    // Animation played when the model is moving
    move_animation: Option<u32>,
    playing_move_animation: bool,
    last_position: DVec3,
    // Animation played when the model is idle
    idle_animation: Option<u32>,
    // New animations
    animation_queue: Vec<Animation>,
    // Animations that are playing
    playing: Vec<Animation>,
}

impl AnimationPlayer {
    pub fn play(&mut self, animation_index: u32) -> &mut Animation {
        self.animation_queue.push(Animation {
            restart: false,
            animation_index,
            repeat: false,
            transition_from: None,
            transition_duration: 0.0,
        });
        self.animation_queue.last_mut().unwrap()
    }

    pub fn stop(&mut self, animation_index: u32) {
        // Animation's always run to completion, but this let's you 'stop' it if it is
        // a repeating animation.
        self.animation_queue.push(Animation {
            restart: false,
            animation_index,
            repeat: false,
            transition_from: None,
            transition_duration: 0.0,
        });
    }

    pub fn set_target(&mut self, target: Entity) {
        self.target = Some(target);
    }

    pub fn set_move_animation(&mut self, animation_index: Option<u32>) {
        if let Some(prev) = self.move_animation.take() {
            self.stop(prev);
        }

        self.move_animation = animation_index;
    }

    pub fn set_idle_animation(&mut self, animation_index: Option<u32>) {
        if let Some(prev) = self.idle_animation.take() {
            self.stop(prev);
        }

        self.idle_animation = animation_index;
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
    #[track_caller]
    pub fn get_by_name(&self, name: &str) -> &ModelConfig {
        if let Some(model) = self.0.get(name) {
            model
        } else {
            panic!(
                "Missing model: '{}', make sure it is added to the assets.",
                name
            );
        }
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
    position2entity: HashMap<ChunkPosition, HashSet<Entity>>,
    entity2position: HashMap<Entity, ChunkPosition>,
}

impl ModelMap {
    pub fn get_entities(&self, chunk_position: &ChunkPosition) -> Option<&HashSet<Entity>> {
        return self.position2entity.get(chunk_position);
    }

    fn insert_or_move(&mut self, chunk_position: ChunkPosition, entity: Entity) {
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
    net: Res<Server>,
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
fn send_model_transform(
    net: Res<Server>,
    chunk_subscriptions: Res<ChunkSubscriptions>,
    mut model_map: ResMut<ModelMap>,
    model_query: Query<
        (Entity, &GlobalTransform, &ModelVisibility, Ref<Model>),
        Changed<GlobalTransform>,
    >,
) {
    for (entity, global_transform, visibility, change_tracker) in model_query.iter() {
        let transform = global_transform.compute_transform();
        let chunk_position = ChunkPosition::from(transform.translation);

        model_map.insert_or_move(chunk_position, entity);

        if !visibility.is_visible() || change_tracker.is_added() {
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

fn apply_animations(mut models: Query<(&mut AnimationPlayer, Ref<GlobalTransform>)>) {
    for (mut animation_player, transform) in models.iter_mut() {
        if animation_player.move_animation.is_some()
            && transform.is_changed()
            // TODO: Even though it doesn't move the translation still changes when the model is
            // rotated! Probably some inaccuracy from converting fram a matrix representation.
            && transform.translation() != animation_player.last_position
        {
            let move_animation = animation_player.move_animation.unwrap();

            let difference = transform
                .translation()
                .xz()
                .distance_squared(animation_player.last_position.xz());

            if !animation_player.playing_move_animation && difference > 0.0005 {
                animation_player.playing_move_animation = true;
                if let Some(idle_animation) = animation_player.idle_animation {
                    animation_player
                        .play(move_animation)
                        .repeat()
                        .transition(idle_animation, 0.25);
                } else {
                    animation_player.play(move_animation).repeat();
                }
            } else if animation_player.playing_move_animation && difference < 0.0005 {
                animation_player.playing_move_animation = false;
                if let Some(idle_animation) = animation_player.idle_animation {
                    animation_player
                        .play(idle_animation)
                        .repeat()
                        .transition(move_animation, 0.25);
                } else {
                    animation_player.stop(move_animation);
                }
            }

            animation_player.last_position = transform.translation();
        }
    }
}

// TODO: I'm not entirely sure what the purpose of this was. Why not just replace the model?
fn update_model_assets(
    net: Res<Server>,
    chunk_subscriptions: Res<ChunkSubscriptions>,
    model_query: Query<(Entity, Ref<Model>, &GlobalTransform, &ModelVisibility), Changed<Model>>,
) {
    for (entity, model, transform, visibility) in model_query.iter() {
        if !visibility.is_visible() || model.is_added() {
            continue;
        }

        let Model::Asset(model_id) = *model else {
            continue;
        };

        let chunk_pos = ChunkPosition::from(transform.translation());

        let subs = match chunk_subscriptions.get_subscribers(&chunk_pos) {
            Some(subs) => subs,
            None => continue,
        };

        net.send_many(
            subs,
            messages::ModelUpdateAsset {
                id: entity.index(),
                asset: model_id,
            },
        );
    }
}

// TODO: Animations must be sent
fn update_visibility(
    net: Res<Server>,
    chunk_subscriptions: Res<ChunkSubscriptions>,
    model_query: Query<
        (Entity, &Model, &ModelVisibility, &GlobalTransform),
        Or<(Changed<ModelVisibility>, Changed<Model>)>,
    >,
) {
    for (entity, model, visibility, transform) in model_query.iter() {
        let transform = transform.compute_transform();

        let chunk_pos = ChunkPosition::from(transform.translation);
        let subs = match chunk_subscriptions.get_subscribers(&chunk_pos) {
            Some(subs) => subs,
            None => continue,
        };

        if visibility.is_visible() {
            match model {
                Model::Asset(model_id) => {
                    net.send_many(
                        subs,
                        messages::NewModel {
                            parent_id: None,
                            id: entity.index(),
                            asset: *model_id,
                            position: transform.translation,
                            rotation: transform.rotation.as_quat(),
                            scale: transform.scale.as_vec3(),
                        },
                    );
                }
                Model::Custom {
                    mesh_indices,
                    mesh_vertices,
                    mesh_normals,
                    material_base_color,
                    material_color_texture,
                    mesh_uvs,
                    material_parallax_texture,
                    material_alpha_mode,
                    material_alpha_cutoff,
                    material_double_sided,
                    // TODO: Collider must be sent to clients
                    collider: _collider,
                } => net.send_many(
                    subs,
                    messages::SpawnCustomModel {
                        id: entity.index(),
                        parent_id: None,
                        position: transform.translation,
                        rotation: transform.rotation.as_quat(),
                        scale: transform.scale.as_vec3(),
                        mesh_indices: mesh_indices.clone(),
                        mesh_vertices: mesh_vertices.clone(),
                        mesh_normals: mesh_normals.clone(),
                        mesh_uvs: mesh_uvs.clone(),
                        material_base_color: material_base_color.clone(),
                        material_color_texture: material_color_texture.clone(),
                        material_parallax_texture: material_parallax_texture.clone(),
                        material_alpha_mode: *material_alpha_mode,
                        material_alpha_cutoff: *material_alpha_cutoff,
                        material_double_sided: *material_double_sided,
                    },
                ),
            }
        } else {
            net.send_many(subs, messages::DeleteModel { id: entity.index() });
        }
    }
}

fn send_models_on_chunk_subscription(
    net: Res<Server>,
    model_map: Res<ModelMap>,
    player_query: Query<Entity, With<Player>>,
    model_query: Query<(
        Option<&Parent>,
        &Model,
        &AnimationPlayer,
        &GlobalTransform,
        &ModelVisibility,
    )>,
    mut chunk_sub_events: EventReader<ChunkSubscriptionEvent>,
) {
    for chunk_sub in chunk_sub_events.read() {
        if let Some(model_entities) = model_map.get_entities(&chunk_sub.chunk_position) {
            for entity in model_entities.iter() {
                let Ok((maybe_player_parent, model, animation_player, transform, visibility)) =
                    model_query.get(*entity)
                else {
                    continue;
                };

                if !visibility.is_visible() {
                    continue;
                }

                // Don't send the player models to the players they belong to.
                if let Some(parent) = maybe_player_parent {
                    let player_entity = player_query.get(parent.get()).unwrap();
                    if player_entity == chunk_sub.player_entity {
                        continue;
                    }
                }

                let transform = transform.compute_transform();

                match model {
                    Model::Asset(model_id) => {
                        net.send_one(
                            chunk_sub.player_entity,
                            messages::NewModel {
                                parent_id: None,
                                id: entity.index(),
                                asset: *model_id,
                                position: transform.translation,
                                rotation: transform.rotation.as_quat(),
                                scale: transform.scale.as_vec3(),
                            },
                        );
                    }
                    Model::Custom {
                        mesh_indices,
                        mesh_vertices,
                        mesh_normals,
                        material_base_color,
                        material_color_texture,
                        mesh_uvs,
                        material_parallax_texture,
                        material_alpha_mode,
                        material_alpha_cutoff,
                        material_double_sided,
                        // TODO: Collider must be sent to clients
                        collider: _collider,
                    } => net.send_one(
                        chunk_sub.player_entity,
                        messages::SpawnCustomModel {
                            id: entity.index(),
                            parent_id: None,
                            position: transform.translation,
                            rotation: transform.rotation.as_quat(),
                            scale: transform.scale.as_vec3(),
                            mesh_indices: mesh_indices.clone(),
                            mesh_vertices: mesh_vertices.clone(),
                            mesh_normals: mesh_normals.clone(),
                            mesh_uvs: mesh_uvs.clone(),
                            material_base_color: material_base_color.clone(),
                            material_color_texture: material_color_texture.clone(),
                            material_parallax_texture: material_parallax_texture.clone(),
                            material_alpha_mode: *material_alpha_mode,
                            material_alpha_cutoff: *material_alpha_cutoff,
                            material_double_sided: *material_double_sided,
                        },
                    ),
                }

                for animation in animation_player.playing.iter() {
                    net.send_one(
                        chunk_sub.player_entity,
                        messages::ModelPlayAnimation {
                            model_id: animation_player.target.unwrap_or(*entity).index(),
                            animation_index: animation.animation_index,
                            restart: animation.restart,
                            repeat: animation.repeat,
                            transition: animation
                                .transition_from
                                .and_then(|from| Some((from, animation.transition_duration))),
                        },
                    );
                }
            }
        }
    }
}

fn send_animations(
    net: Res<Server>,
    chunk_subscriptions: Res<ChunkSubscriptions>,
    mut animation_query: Query<
        (Entity, &mut AnimationPlayer, &GlobalTransform),
        Changed<AnimationPlayer>,
    >,
) {
    for (entity, mut animation_player, transform) in animation_query.iter_mut() {
        let chunk_position = ChunkPosition::from(transform.translation());

        let Some(subs) = chunk_subscriptions.get_subscribers(&chunk_position) else {
            animation_player.animation_queue.clear();
            continue;
        };

        // split borrow
        let animation_player = animation_player.into_inner();

        for animation in animation_player.animation_queue.drain(..) {
            net.send_many(
                subs,
                messages::ModelPlayAnimation {
                    model_id: animation_player.target.unwrap_or(entity).index(),
                    animation_index: animation.animation_index,
                    restart: animation.restart,
                    repeat: animation.repeat,
                    transition: animation
                        .transition_from
                        .and_then(|from| Some((from, animation.transition_duration))),
                },
            );
        }
    }
}
