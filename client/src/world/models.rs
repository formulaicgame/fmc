use std::{
    collections::{HashMap, HashSet},
    time::Duration,
};

use bevy::{
    animation::{ActiveAnimation, RepeatAnimation},
    gltf::Gltf,
    math::DVec3,
    pbr::{ExtendedMaterial, NotShadowCaster},
    prelude::*,
    render::{
        mesh::{Indices, MeshAabb},
        primitives::Aabb,
        render_asset::RenderAssetUsages,
    },
};
use fmc_protocol::messages;

use crate::{
    assets::models::{Model, Models},
    game_state::GameState,
    networking::NetworkClient,
    rendering::materials::ModelMaterial,
    world::{MovesWithOrigin, Origin},
};

pub struct ModelPlugin;
impl Plugin for ModelPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ModelEntities::default())
            .add_systems(
                PostUpdate,
                (advance_transitions, expire_completed_transitions),
            )
            .add_systems(
                Update,
                (
                    handle_model_add_delete,
                    handle_custom_models,
                    update_model_asset,
                    //render_aabb,
                    handle_transform_updates,
                    handle_model_color,
                    interpolation,
                    //advance_transitions,
                    play_animations.after(handle_model_add_delete),
                )
                    .run_if(in_state(GameState::Playing)),
            );
    }
}

// TODO: Small fix from bevy's implementation, will probably be fixed in 0.16
#[derive(Component, Default, Reflect)]
#[reflect(Component, Default)]
pub struct AnimationTransitions {
    main_animation: Option<AnimationNodeIndex>,
    transitions: Vec<AnimationTransition>,
}

// This is needed since `#[derive(Clone)]` does not generate optimized `clone_from`.
impl Clone for AnimationTransitions {
    fn clone(&self) -> Self {
        Self {
            main_animation: self.main_animation,
            transitions: self.transitions.clone(),
        }
    }

    fn clone_from(&mut self, source: &Self) {
        self.main_animation = source.main_animation;
        self.transitions.clone_from(&source.transitions);
    }
}

/// An animation that is being faded out as part of a transition
#[derive(Debug, Clone, Copy, Reflect)]
pub struct AnimationTransition {
    /// The current weight. Starts at 1.0 and goes to 0.0 during the fade-out.
    current_weight: f32,
    /// How much to decrease `current_weight` per second
    weight_decline_per_sec: f32,
    /// The animation that is being faded out
    animation: AnimationNodeIndex,
}

impl AnimationTransitions {
    /// Creates a new [`AnimationTransitions`] component, ready to be added to
    /// an entity with an [`AnimationPlayer`].
    pub fn new() -> AnimationTransitions {
        AnimationTransitions::default()
    }

    /// Plays a new animation on the given [`AnimationPlayer`], fading out any
    /// existing animations that were already playing over the
    /// `transition_duration`.
    ///
    /// Pass [`Duration::ZERO`] to instantly switch to a new animation, avoiding
    /// any transition.
    pub fn play<'p>(
        &mut self,
        player: &'p mut AnimationPlayer,
        new_animation: AnimationNodeIndex,
        transition_duration: Duration,
    ) -> &'p mut ActiveAnimation {
        if let Some(old_animation_index) = self.main_animation.replace(new_animation) {
            if let Some(old_animation) = player.animation_mut(old_animation_index) {
                if !old_animation.is_paused() {
                    self.transitions.push(AnimationTransition {
                        current_weight: old_animation.weight(),
                        weight_decline_per_sec: 1.0 / transition_duration.as_secs_f32(),
                        animation: old_animation_index,
                    });
                }
            }
        }

        // If already transitioning away from this animation, cancel the transition.
        // Otherwise the transition ending would incorrectly stop the new animation.
        self.transitions
            .retain(|transition| transition.animation != new_animation);

        player.start(new_animation)
    }

    /// Obtain the currently playing main animation.
    pub fn get_main_animation(&self) -> Option<AnimationNodeIndex> {
        self.main_animation
    }
}

/// A system that alters the weight of currently-playing transitions based on
/// the current time and decline amount.
pub fn advance_transitions(
    mut query: Query<(&mut AnimationTransitions, &mut AnimationPlayer)>,
    time: Res<Time>,
) {
    // We use a "greedy layer" system here. The top layer (most recent
    // transition) gets as much as weight as it wants, and the remaining amount
    // is divided between all the other layers, eventually culminating in the
    // currently-playing animation receiving whatever's left. This results in a
    // nicely normalized weight.
    for (mut animation_transitions, mut player) in query.iter_mut() {
        let mut remaining_weight = 1.0;
        for transition in &mut animation_transitions.transitions.iter_mut().rev() {
            // Decrease weight.
            transition.current_weight = (transition.current_weight
                - transition.weight_decline_per_sec * time.delta_secs())
            .max(0.0);

            // Update weight.
            let Some(ref mut animation) = player.animation_mut(transition.animation) else {
                continue;
            };
            animation.set_weight(transition.current_weight * remaining_weight);
            remaining_weight -= animation.weight();
        }

        if let Some(main_animation_index) = animation_transitions.main_animation {
            if let Some(ref mut animation) = player.animation_mut(main_animation_index) {
                animation.set_weight(remaining_weight);
            }
        }
    }
}

/// A system that removed transitions that have completed from the
/// [`AnimationTransitions`] object.
pub fn expire_completed_transitions(
    mut query: Query<(&mut AnimationTransitions, &mut AnimationPlayer)>,
) {
    for (mut animation_transitions, mut player) in query.iter_mut() {
        animation_transitions.transitions.retain(|transition| {
            let expire = transition.current_weight <= 0.0;
            if expire {
                player.stop(transition.animation);
            }
            !expire
        });
    }
}

/// Map from server model id to entity
#[derive(Resource, Default)]
pub struct ModelEntities {
    id2entity: HashMap<u32, Entity>,
    entity2id: HashMap<Entity, u32>,
}

impl ModelEntities {
    fn insert(&mut self, model_id: u32, entity: Entity) {
        self.id2entity.insert(model_id, entity);
        self.entity2id.insert(entity, model_id);
    }

    fn remove(&mut self, model_id: u32) -> Option<Entity> {
        let entity = self.id2entity.remove(&model_id)?;
        self.entity2id.remove(&entity);
        Some(entity)
    }

    pub fn get_entity(&self, model_id: &u32) -> Option<Entity> {
        self.id2entity.get(model_id).cloned()
    }

    pub fn get_model_id(&self, entity: &Entity) -> Option<u32> {
        self.entity2id.get(entity).cloned()
    }

    pub fn drain(&mut self) -> Vec<Entity> {
        self.id2entity.clear();
        self.entity2id.drain().map(|(k, _)| k).collect()
    }
}

fn handle_model_add_delete(
    net: Res<NetworkClient>,
    mut commands: Commands,
    origin: Res<Origin>,
    models: Res<Models>,
    gltf_assets: Res<Assets<Gltf>>,
    mut model_entities: ResMut<ModelEntities>,
    mut deleted_models: EventReader<messages::DeleteModel>,
    mut new_models: EventReader<messages::NewModel>,
) {
    for deleted_model in deleted_models.read() {
        if let Some(entity) = model_entities.remove(deleted_model.model_id) {
            commands.entity(entity).despawn();
        }
    }

    for new_model in new_models.read() {
        // Server may send same id with intent to replace, in which case we delete and add anew
        if let Some(old_entity) = model_entities.remove(new_model.model_id) {
            commands.entity(old_entity).despawn();
        }

        let Some(model_config) = models.get_config(&new_model.asset) else {
            net.disconnect(format!(
                "Server sent model asset id that doesn't exist, id: {}",
                new_model.asset,
            ));
            return;
        };

        let Some(gltf) = gltf_assets.get(&model_config.gltf_handle) else {
            continue;
        };

        let entity = commands
            .spawn((
                SceneRoot(gltf.scenes[0].clone()),
                Transform {
                    translation: origin.to_local(new_model.position),
                    rotation: new_model.rotation,
                    scale: new_model.scale,
                },
                Model::Asset(new_model.asset),
                AnimationGraphHandle(model_config.animation_graph.clone().unwrap()),
                AnimationPlayer::default(),
                AnimationTransitions::default(),
                TransformInterpolation::default(),
                MovesWithOrigin,
            ))
            .id();

        model_entities.insert(new_model.model_id, entity);
    }
}

// The asset server will unload unused assets so we keep them here after first load to minimize
// flickering from loading time.
#[derive(Default, Deref, DerefMut)]
struct TextureCache(HashSet<Handle<Image>>);

fn handle_custom_models(
    asset_server: Res<AssetServer>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut commands: Commands,
    origin: Res<Origin>,
    mut model_entities: ResMut<ModelEntities>,
    mut new_models: EventReader<messages::SpawnCustomModel>,
    mut cache: Local<TextureCache>,
) {
    for custom_model in new_models.read() {
        // Server may send same id with intent to replace, in which case we delete and add anew
        if let Some(old_entity) = model_entities.remove(custom_model.model_id) {
            commands.entity(old_entity).despawn();
        }

        let mut mesh = Mesh::new(
            bevy::render::mesh::PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        );

        mesh.insert_indices(Indices::U32(custom_model.mesh_indices.clone()));
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, custom_model.mesh_vertices.clone());
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, custom_model.mesh_normals.clone());

        if let Some(texture_uvs) = &custom_model.mesh_uvs {
            mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, texture_uvs.clone());
        }

        const TEXTURE_PATH: &str = "server_assets/active/textures/";

        let base_color_texture = custom_model.material_color_texture.as_ref().map(|path| {
            let handle = asset_server.load(TEXTURE_PATH.to_owned() + &path);
            cache.insert(handle.clone());
            handle
        });

        let depth_map = custom_model.material_parallax_texture.as_ref().map(|path| {
            let handle = asset_server.load(TEXTURE_PATH.to_owned() + &path);
            cache.insert(handle.clone());
            handle
        });

        let material = StandardMaterial {
            base_color_texture,
            depth_map,
            alpha_mode: match custom_model.material_alpha_mode {
                0 => AlphaMode::Opaque,
                1 => AlphaMode::Mask(custom_model.material_alpha_cutoff),
                2 => AlphaMode::Blend,
                // TODO: Disconnect
                _ => AlphaMode::Opaque,
            },
            double_sided: custom_model.material_double_sided,
            ..default()
        };

        let entity = commands
            .spawn((
                Model::Custom {
                    aabb: mesh
                        .compute_aabb()
                        .expect("mesh to have ATTRIBUTE_POSITION"),
                },
                Mesh3d(meshes.add(mesh)),
                MeshMaterial3d(materials.add(material)),
                Transform {
                    translation: origin.to_local(custom_model.position),
                    rotation: custom_model.rotation,
                    scale: custom_model.scale,
                },
                AnimationPlayer::default(),
                TransformInterpolation::default(),
                MovesWithOrigin,
            ))
            .id();

        model_entities.insert(custom_model.model_id, entity);
    }
}

fn update_model_asset(
    net: Res<NetworkClient>,
    model_entities: Res<ModelEntities>,
    models: Res<Models>,
    gltf_assets: Res<Assets<Gltf>>,
    mut model_query: Query<(&mut SceneRoot, &mut AnimationGraphHandle, &mut Model)>,
    mut asset_updates: EventReader<messages::ModelUpdateAsset>,
) {
    for asset_update in asset_updates.read() {
        if let Some(entity) = model_entities.get_entity(&asset_update.model_id) {
            let (mut scene, mut animation_graph, mut model) = model_query.get_mut(entity).unwrap();

            let Some(model_config) = models.get_config(&asset_update.asset) else {
                net.disconnect(format!(
                    "Server sent model asset id that doesn't exist, id: {}",
                    asset_update.model_id
                ));
                return;
            };

            *scene =
                SceneRoot(gltf_assets.get(&model_config.gltf_handle).unwrap().scenes[0].clone());
            *model = Model::Asset(asset_update.asset);
            *animation_graph = AnimationGraphHandle(model_config.animation_graph.clone().unwrap());
        }
    }
}

#[derive(Component)]
struct TransformInterpolation {
    progress: f32,
    translation: DVec3,
    rotation: Quat,
    scale: Vec3,
}

impl Default for TransformInterpolation {
    fn default() -> Self {
        Self {
            progress: 1.0,
            translation: DVec3::default(),
            rotation: Quat::default(),
            scale: Vec3::default(),
        }
    }
}

fn handle_transform_updates(
    model_entities: Res<ModelEntities>,
    mut transform_updates: EventReader<messages::ModelUpdateTransform>,
    mut model_query: Query<&mut TransformInterpolation, With<Model>>,
) {
    for transform_update in transform_updates.read() {
        if let Some(entity) = model_entities.get_entity(&transform_update.model_id) {
            // TODO: I think this should be bug, server should not send model same tick it sends
            // transform updated. But there is 1-frame delay for model entity spawn for command
            // application. Should be disconnect I think, if bevy ever gets immediate command
            // application.
            let mut interpolation = match model_query.get_mut(entity) {
                Ok(m) => m,
                Err(_) => continue,
            };

            interpolation.translation = transform_update.position;
            interpolation.rotation = transform_update.rotation;
            interpolation.scale = transform_update.scale;
            interpolation.progress = 0.0;
        }
    }
}

fn interpolation(
    origin: Res<Origin>,
    mut model_query: Query<
        (&mut Transform, &mut TransformInterpolation),
        (
            With<Model>,
            Or<(Changed<GlobalTransform>, Changed<TransformInterpolation>)>,
        ),
    >,
) {
    for (mut transform, mut interpolation) in model_query.iter_mut() {
        interpolation.progress += 1.0 / 6.0;
        if interpolation.progress > 1.0 {
            continue;
        }

        let interpolation_transform = Transform {
            translation: (interpolation.translation - origin.as_dvec3()).as_vec3(),
            rotation: interpolation.rotation,
            scale: interpolation.scale,
        };

        let new_transform = Animatable::interpolate(
            &*transform,
            &interpolation_transform,
            interpolation.progress,
        );

        transform.set_if_neq(new_transform);
    }
}

fn play_animations(
    net: Res<NetworkClient>,
    models: Res<Models>,
    model_entities: Res<ModelEntities>,
    mut model_query: Query<
        (&mut Model, &mut AnimationPlayer, &mut AnimationTransitions),
        With<AnimationGraphHandle>,
    >,
    mut animation_events: EventReader<messages::ModelPlayAnimation>,
) {
    for animation in animation_events.read() {
        let Some(model_entity) = model_entities.get_entity(&animation.model_id) else {
            // net.disconnect(
            //     "The server tried to play an animation for an entity that doesn't exist.",
            // );
            return;
        };

        let (model, mut animation_player, mut transition) =
            model_query.get_mut(model_entity).unwrap();

        let Model::Asset(model_asset_id) = *model else {
            // TODO: Disconnect
            continue;
        };

        let model_config = models.get_config(&model_asset_id).unwrap();

        let Some(animation_index) = model_config
            .animations
            .get(animation.animation_index as usize)
        else {
            // TODO: Need to print the name of the model in the error message for debugging.
            // net.disconnect(format!(
            //     "The server sent an animation that doesn't exist. Animation index was '{}'",
            //     animation.animation_index
            // ));
            return;
        };

        let active_animation = if let Some((from_animation, duration)) = animation.transition {
            let Some(from_animation_index) = model_config.animations.get(from_animation as usize)
            else {
                // TODO: Need to print the name of the model in the error message for debugging.
                // net.disconnect(format!(
                //     "The server sent an animation that doesn't exist. Animation index was '{}'",
                //     animation.animation_index
                // ));
                return;
            };

            transition.play(
                &mut animation_player,
                //*from_animation_index,
                *animation_index,
                Duration::from_secs_f32(duration),
            )
        } else {
            animation_player.play(*animation_index)
        };

        if active_animation.is_finished() || animation.restart {
            active_animation.replay();
        }

        // When the server wants an animation to stop, it sends the same animation but with
        // repeat=false. Then we complete the current animation cycle and stop.
        if animation.repeat {
            active_animation.repeat();
        } else {
            // TODO: It messes up the last frame
            // https://github.com/bevyengine/bevy/issues/10832
            let count = active_animation.completions() + 1;
            active_animation.set_repeat(RepeatAnimation::Count(count));
        }
    }
}

fn handle_model_color(
    net: Res<NetworkClient>,
    children_query: Query<&Children>,
    material_query: Query<&MeshMaterial3d<ModelMaterial>>,
    model_entities: Res<ModelEntities>,
    mut materials: ResMut<Assets<ModelMaterial>>,
    mut color_updates: EventReader<messages::ModelColor>,
) {
    fn change_color(
        color: Color,
        entity: Entity,
        material_query: &Query<&MeshMaterial3d<ModelMaterial>>,
        children_query: &Query<&Children>,
        materials: &mut Assets<ModelMaterial>,
    ) {
        if let Ok(material_handle) = material_query.get(entity) {
            let material = materials.get_mut(material_handle).unwrap();
            material.base.base_color = color;
        };

        if let Ok(children) = children_query.get(entity) {
            for child in children {
                change_color(color, *child, material_query, children_query, materials);
            }
        }
    }

    for message in color_updates.read() {
        let color = match Srgba::hex(&message.color) {
            Ok(c) => c,
            Err(e) => {
                net.disconnect(format!(
                    "Recevied malformed material color '{}', error: {}",
                    message.color, e
                ));
                return;
            }
        };

        let Some(model_entity) = model_entities.get_entity(&message.model_id) else {
            // TODO: Disconnect
            return;
        };

        change_color(
            color.into(),
            model_entity,
            &material_query,
            &children_query,
            &mut materials,
        );
    }
}

fn render_aabb(
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    aabb_query: Query<(Entity, &Aabb, &Transform), (With<Model>, Added<Aabb>)>,
) {
    for (entity, aabb, transform) in aabb_query.iter() {
        let max = aabb.max();
        let min = aabb.min();
        /*
              (2)-----(3)               Y
               | \     | \              |
               |  (1)-----(0) MAX       o---X
               |   |   |   |             \
          MIN (6)--|--(7)  |              Z
                 \ |     \ |
                  (5)-----(4)
        */
        let vertices = vec![
            [max.x, max.y, max.z],
            [min.x, max.y, max.z],
            [min.x, max.y, min.z],
            [max.x, max.y, min.z],
            [max.x, min.y, max.z],
            [min.x, min.y, max.z],
            [min.x, min.y, min.z],
            [max.x, min.y, min.z],
        ];

        let indices = Indices::U32(vec![
            0, 1, 1, 2, 2, 3, 3, 0, // Top
            4, 5, 5, 6, 6, 7, 7, 4, // Bottom
            0, 4, 1, 5, 2, 6, 3, 7, // Verticals
        ]);

        let mut mesh = Mesh::new(
            bevy::render::render_resource::PrimitiveTopology::LineList,
            RenderAssetUsages::default(),
        );
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, vertices.clone());
        mesh.insert_indices(indices);

        let child = commands
            .spawn((
                Mesh3d(meshes.add(mesh)),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: Color::srgb(0.0, 1.0, 0.0),
                    unlit: true,
                    ..default()
                })),
                Transform {
                    scale: 1.0 / transform.scale,
                    translation: Vec3::new(0.0, 0.0, 0.0),
                    ..default()
                },
                NotShadowCaster,
            ))
            .id();
        commands.entity(entity).add_child(child);
    }
}
