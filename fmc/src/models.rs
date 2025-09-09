use std::collections::{HashMap, HashSet};

use bevy::{math::DVec3, prelude::*};
use fmc_protocol::messages;

use crate::{
    assets::AssetSet,
    bevy_extensions::f64_transform::{GlobalTransform, Transform, TransformSystem},
    database::Database,
    networking::Server,
    physics::Collider,
    players::Player,
    world::{ChunkOrigin, ChunkSubscriptionEvent, ChunkSubscriptions, chunk::ChunkPosition},
};

pub const MODEL_PATH: &str = "./assets/client/textures/models/";

// Used to identify the asset of a model.
pub type ModelAssetId = u32;

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
                    apply_movement_animations
                        .before(send_animations)
                        .after(TransformSystem::TransformPropagate),
                    send_animations,
                    send_color,
                    remove_models.after(send_model_transform),
                    send_model_transform.after(TransformSystem::TransformPropagate),
                    send_models
                        .before(send_animations)
                        .after(TransformSystem::TransformPropagate),
                )
                    .in_set(ModelSystems),
            );
    }
}

#[derive(Debug, Hash, PartialEq, Eq, Clone, SystemSet)]
pub struct ModelSystems;

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
    let mut model_configs = HashMap::with_capacity(directory.size_hint().0);

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
            // XXX: We change the id to the correct one when moving the configs into Models.
            id: 0,
            animations: HashMap::new(),
            collider: Collider::default(),
            meshes: Vec::new(),
        };

        if extension == "json" {
            // Block models can be defined through json files.
            config.collider =
                Collider::from_min_max(DVec3::new(-0.5, 0.0, -0.5), DVec3::new(0.5, 1.0, 0.5));

            // TODO: Remove and define in the json file. Let them have parents so you don't have to
            // copy the animations all over. There is probably even some reason to have a custom
            // format for models beyond gltf.
            config.animations.insert("left_click".to_owned(), 0);
            config.animations.insert("equip".to_owned(), 1);
            config.animations.insert("dropped".to_owned(), 2);
        } else if extension == "glb" || extension == "gltf" {
            let (gltf, buffers, _) = match gltf::import(&path) {
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

                let mut model_mesh = ModelMesh::default();
                for primitive in mesh.primitives() {
                    let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));
                    if let Some(iter) = reader.read_positions() {
                        for vertex in iter {
                            let vertex = Vec3::from_array(vertex) + translation;
                            model_mesh.vertices.push(vertex.to_array());
                        }
                    }

                    if let Some(indices) = reader.read_indices() {
                        match indices {
                            gltf::mesh::util::ReadIndices::U8(indices) => {
                                model_mesh.indices.extend(indices.map(|index| index as u32))
                            }
                            gltf::mesh::util::ReadIndices::U16(indices) => {
                                model_mesh.indices.extend(indices.map(|index| index as u32))
                            }
                            gltf::mesh::util::ReadIndices::U32(indices) => {
                                model_mesh.indices.extend(indices)
                            }
                        }
                    }

                    if let Some(normals) = reader.read_normals() {
                        model_mesh.normals.extend(normals);
                    }

                    if let Some(uvs) = reader.read_tex_coords(0) {
                        match uvs {
                            gltf::mesh::util::ReadTexCoords::F32(iter) => {
                                model_mesh.uvs.extend(iter)
                            }
                            // TODO: Idk what to do with the others
                            _ => (),
                        }
                    }

                    let bounds = primitive.bounding_box();
                    min = min.min(Vec3::from_array(bounds.min) + translation);
                    max = max.max(Vec3::from_array(bounds.max) + translation);
                }

                config.meshes.push(model_mesh);
            }

            config.collider = Collider::from_min_max(min.as_dvec3(), max.as_dvec3());

            for animation in gltf.animations() {
                if let Some(name) = animation.name() {
                    config
                        .animations
                        .insert(name.to_owned(), animation.index() as u32);
                }
            }
        } else {
            panic!("Invalid model");
        }

        // TODO: These unwraps can probably fail
        let name = path.file_stem().unwrap().to_str().unwrap().to_lowercase();

        model_configs.insert(name, config);
    }

    let model_ids = database.load_model_ids();

    let mut models = Models {
        configs: Vec::with_capacity(model_ids.len()),
        ids: HashMap::with_capacity(model_ids.len()),
    };

    for (model_id, model_name) in model_ids.into_iter().enumerate() {
        let Some(mut config) = model_configs.remove(&model_name) else {
            panic!(
                "Missing model '{}', make sure it exists at '{}' as a gltf/glb/json file",
                model_name, MODEL_PATH
            );
        };

        let id = model_id as ModelAssetId;
        config.id = id;
        models.configs.push(config);

        models.ids.insert(model_name, id);
    }

    commands.insert_resource(models);
}

// TODO: Setting the default move animation is almost always something you want to do, but only on
// initial spawn. Maybe introduce a transient component in this bundle that can be removed when
// added.
// TODO: With "custom" this is almost 200 bytes per model
#[derive(Component)]
#[require(ModelVisibility, AnimationPlayer, Transform, ChunkOrigin)]
pub enum Model {
    Asset(ModelAssetId),
    Custom {
        /// Mesh Indices
        mesh_indices: Vec<u32>,
        /// Mesh vertices
        mesh_vertices: Vec<[f32; 3]>,
        /// Mesh normals
        mesh_normals: Vec<[f32; 3]>,
        /// Texture uvs
        mesh_uvs: Option<Vec<[f32; 2]>>,
        /// Color texture of the mesh
        material_color_texture: Option<String>,
        /// Texture used for parallax mapping
        material_parallax_texture: Option<String>,
        /// Alpha blend mode, 0 = Opaque, 1 = mask, 2 = blend
        material_alpha_mode: u8,
        /// Alpha channel cutoff if the blend mode is Mask
        material_alpha_cutoff: f32,
        /// Render mesh from both sides
        material_double_sided: bool,
    },
}

#[derive(Component, PartialEq)]
pub struct ModelColor {
    red: f32,
    green: f32,
    blue: f32,
    alpha: f32,
}

impl ModelColor {
    pub const WHITE: Self = Self::new(1.0, 1.0, 1.0, 1.0);

    pub const fn new(red: f32, green: f32, blue: f32, alpha: f32) -> Self {
        Self {
            red,
            green,
            blue,
            alpha,
        }
    }

    fn to_hex(&self) -> String {
        let [r, g, b, a] = [self.red, self.green, self.blue, self.alpha]
            .map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8);

        format!("#{:02X}{:02X}{:02X}{:02X}", r, g, b, a)
    }
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
    // Transition time between move and idle animation
    transition_time: f32,
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

    /// Animations always run to completion, but this lets you 'stop' one if it is
    /// a repeating animation.
    pub fn stop(&mut self, animation_index: u32) {
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
        if self.move_animation == animation_index {
            return;
        }

        if let Some(prev) = self.move_animation.take() {
            if self.playing_move_animation {
                if let Some(new) = animation_index {
                    let time = self.transition_time;
                    self.play(new).repeat().transition(prev, time);
                } else {
                    self.stop(prev);
                }
            }
        }

        self.move_animation = animation_index;
    }

    pub fn set_idle_animation(&mut self, animation_index: Option<u32>) {
        if self.idle_animation == animation_index {
            return;
        }

        if let Some(prev) = self.idle_animation.take() {
            if !self.playing_move_animation {
                if let Some(new) = animation_index {
                    let time = self.transition_time;
                    self.play(new).repeat().transition(prev, time);
                } else {
                    self.stop(prev);
                }
            }
        }

        self.idle_animation = animation_index;
    }

    /// Set the transition time between the move and the idle animation
    pub fn set_transition_time(&mut self, duration: f32) {
        self.transition_time = duration;
    }
}

#[derive(Default)]
pub struct ModelMesh {
    pub vertices: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    pub indices: Vec<u32>,
}

pub struct ModelConfig {
    pub id: ModelAssetId,
    // Map from animation name (as stored in the gltf file) to its index
    pub animations: HashMap<String, u32>,
    pub collider: Collider,
    pub meshes: Vec<ModelMesh>,
}

// The models are stored as an IndexMap where the index corresponds to the model's asset id.
#[derive(Resource)]
pub struct Models {
    configs: Vec<ModelConfig>,
    ids: HashMap<String, ModelAssetId>,
}

impl Models {
    #[track_caller]
    pub fn get_config_by_name(&self, name: &str) -> Option<&ModelConfig> {
        let id = self.ids.get(name)?;
        return self.configs.get(*id as usize);
    }

    pub fn get_config(&self, id: &ModelAssetId) -> &ModelConfig {
        &self.configs[*id as usize]
    }

    pub fn get_id(&self, name: &str) -> Option<ModelAssetId> {
        return self.ids.get(name).cloned();
    }

    pub fn ids(&self) -> &HashMap<String, ModelAssetId> {
        return &self.ids;
    }

    pub fn configs(&self) -> &Vec<ModelConfig> {
        return &self.configs;
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
                self.position2entity
                    .get_mut(current_chunk_pos)
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
        let Some(chunk_position) = model_map.entity2position.remove(&entity) else {
            // TODO: This if condition can be removed, I just want to test for a while that I didn't
            // mess up.
            error!("Despawned model was not entered in the model map, this should never happen.");
            continue;
        };

        let chunk = model_map.position2entity.get_mut(&chunk_position).unwrap();
        chunk.remove(&entity);

        if chunk.is_empty() {
            model_map.position2entity.remove(&chunk_position);
        }

        if let Some(subs) = chunk_subscriptions.get_subscribers(&chunk_position) {
            net.send_many(
                subs,
                messages::DeleteModel {
                    model_id: entity.index(),
                },
            );
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
                model_id: entity.index(),
                position: transform.translation,
                rotation: transform.rotation.as_quat(),
                scale: transform.scale.as_vec3(),
            },
        );
    }
}

fn apply_movement_animations(
    time: Res<Time>,
    mut models: Query<(&mut AnimationPlayer, Ref<GlobalTransform>)>,
) {
    for (mut animation_player, transform) in models.iter_mut() {
        if animation_player.move_animation.is_some()
            && transform.is_changed()
            // TODO: Even though it doesn't move the translation still changes when the model is
            // rotated! Probably some inaccuracy from converting fram a matrix representation.
            && transform.translation() != animation_player.last_position
        {
            let move_animation = animation_player.move_animation.unwrap();

            let speed = transform
                .translation()
                .xz()
                .distance_squared(animation_player.last_position.xz())
                / time.delta_secs_f64();

            if !animation_player.playing_move_animation && speed > 0.002 {
                animation_player.playing_move_animation = true;
                if let Some(idle_animation) = animation_player.idle_animation
                    && animation_player.transition_time != 0.0
                {
                    let transition_time = animation_player.transition_time;
                    let mut animation = animation_player
                        .play(move_animation)
                        .repeat()
                        .transition(idle_animation, transition_time);
                } else {
                    animation_player.play(move_animation).repeat();
                }
            } else if animation_player.playing_move_animation && speed < 0.002 {
                animation_player.playing_move_animation = false;
                if let Some(idle_animation) = animation_player.idle_animation
                    && animation_player.transition_time != 0.0
                {
                    let transition_time = animation_player.transition_time;
                    animation_player
                        .play(idle_animation)
                        .repeat()
                        .transition(move_animation, transition_time);
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
                model_id: entity.index(),
                asset: model_id,
            },
        );
    }
}

// TODO: Animations must be sent
fn send_models(
    net: Res<Server>,
    chunk_subscriptions: Res<ChunkSubscriptions>,
    player_query: Query<Entity, With<Player>>,
    model_query: Query<
        (
            Entity,
            Option<&ChildOf>,
            &Model,
            &ModelVisibility,
            Option<&ModelColor>,
            &GlobalTransform,
        ),
        Or<(Changed<ModelVisibility>, Changed<Model>)>,
    >,
) {
    for (entity, maybe_parent, model, visibility, maybe_color, transform) in model_query.iter() {
        let transform = transform.compute_transform();

        // Don't send the player model to the player it belongs to.
        let player_entity = if let Some(parent) = maybe_parent {
            if let Ok(player_entity) = player_query.get(parent.0) {
                Some(player_entity)
            } else {
                None
            }
        } else {
            None
        };

        let chunk_pos = ChunkPosition::from(transform.translation);
        let subs = match chunk_subscriptions.get_subscribers(&chunk_pos) {
            Some(subs) => subs,
            None => continue,
        };

        if visibility.is_visible() {
            match model {
                Model::Asset(model_id) => {
                    net.send_many(
                        subs.iter().filter(|sub| Some(**sub) != player_entity),
                        messages::NewModel {
                            parent_id: None,
                            model_id: entity.index(),
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
                    material_color_texture,
                    mesh_uvs,
                    material_parallax_texture,
                    material_alpha_mode,
                    material_alpha_cutoff,
                    material_double_sided,
                } => net.send_many(
                    subs.iter().filter(|sub| Some(**sub) != player_entity),
                    messages::SpawnCustomModel {
                        model_id: entity.index(),
                        parent_id: None,
                        position: transform.translation,
                        rotation: transform.rotation.as_quat(),
                        scale: transform.scale.as_vec3(),
                        mesh_indices: mesh_indices.clone(),
                        mesh_vertices: mesh_vertices.clone(),
                        mesh_normals: mesh_normals.clone(),
                        mesh_uvs: mesh_uvs.clone(),
                        material_color_texture: material_color_texture.clone(),
                        material_parallax_texture: material_parallax_texture.clone(),
                        material_alpha_mode: *material_alpha_mode,
                        material_alpha_cutoff: *material_alpha_cutoff,
                        material_double_sided: *material_double_sided,
                    },
                ),
            }

            if let Some(color) = maybe_color {
                net.send_many(
                    subs.iter().filter(|sub| Some(**sub) != player_entity),
                    messages::ModelColor {
                        model_id: entity.index(),
                        color: color.to_hex(),
                    },
                );
            }
        } else {
            net.send_many(
                subs.iter().filter(|sub| Some(**sub) != player_entity),
                messages::DeleteModel {
                    model_id: entity.index(),
                },
            );
        }
    }
}

fn send_models_on_chunk_subscription(
    net: Res<Server>,
    model_map: Res<ModelMap>,
    player_query: Query<Entity, With<Player>>,
    model_query: Query<(
        Option<&ChildOf>,
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

                // Don't send the player model to the player it belongs to.
                if let Some(parent) = maybe_player_parent {
                    let player_entity = player_query.get(parent.0).unwrap();
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
                                model_id: entity.index(),
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
                        material_color_texture,
                        mesh_uvs,
                        material_parallax_texture,
                        material_alpha_mode,
                        material_alpha_cutoff,
                        material_double_sided,
                    } => net.send_one(
                        chunk_sub.player_entity,
                        messages::SpawnCustomModel {
                            model_id: entity.index(),
                            parent_id: None,
                            position: transform.translation,
                            rotation: transform.rotation.as_quat(),
                            scale: transform.scale.as_vec3(),
                            mesh_indices: mesh_indices.clone(),
                            mesh_vertices: mesh_vertices.clone(),
                            mesh_normals: mesh_normals.clone(),
                            mesh_uvs: mesh_uvs.clone(),
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

fn send_color(
    net: Res<Server>,
    chunk_subscriptions: Res<ChunkSubscriptions>,
    color_query: Query<(Entity, &ModelColor, &GlobalTransform), Changed<ModelColor>>,
) {
    for (entity, color, transform) in color_query.iter() {
        let chunk_position = ChunkPosition::from(transform.translation());

        let Some(subs) = chunk_subscriptions.get_subscribers(&chunk_position) else {
            continue;
        };

        net.send_many(
            subs,
            messages::ModelColor {
                model_id: entity.index(),
                color: color.to_hex(),
            },
        )
    }
}
