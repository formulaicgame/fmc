use std::collections::{HashMap, HashSet};

use bevy::{
    animation::RepeatAnimation,
    gltf::Gltf,
    math::DVec3,
    pbr::NotShadowCaster,
    prelude::*,
    render::{mesh::Indices, primitives::Aabb, render_asset::RenderAssetUsages},
};
use fmc_protocol::messages;

use crate::{
    assets::models::{Model, Models},
    game_state::GameState,
    networking::NetworkClient,
    world::{MovesWithOrigin, Origin},
};

pub struct ModelPlugin;
impl Plugin for ModelPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ModelEntities::default()).add_systems(
            Update,
            (
                handle_model_add_delete,
                handle_custom_models,
                update_model_asset,
                //render_aabb,
                handle_transform_updates,
                interpolate_to_new_transform,
                play_animations.after(handle_model_add_delete),
            )
                .run_if(in_state(GameState::Playing)),
        );
    }
}

/// Map from server model id to entity
#[derive(Resource, Deref, DerefMut, Default)]
struct ModelEntities(HashMap<u32, Entity>);

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
        if let Some(entity) = model_entities.remove(&deleted_model.id) {
            commands.entity(entity).despawn_recursive();
        }
    }

    for new_model in new_models.read() {
        // Server may send same id with intent to replace, in which case we delete and add anew
        if let Some(old_entity) = model_entities.remove(&new_model.id) {
            commands.entity(old_entity).despawn_recursive();
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
                TransformInterpolation::default(),
                MovesWithOrigin,
            ))
            .id();

        model_entities.insert(new_model.id, entity);
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
        if let Some(old_entity) = model_entities.remove(&custom_model.id) {
            commands.entity(old_entity).despawn_recursive();
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
            base_color: Srgba::hex(&custom_model.material_base_color)
                .unwrap_or(Srgba::WHITE)
                .into(),
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
                Mesh3d(meshes.add(mesh)),
                MeshMaterial3d(materials.add(material)),
                Transform {
                    translation: (custom_model.position - origin.as_dvec3()).as_vec3(),
                    rotation: custom_model.rotation,
                    scale: custom_model.scale,
                },
                Model::Custom,
                AnimationPlayer::default(),
                TransformInterpolation::default(),
                MovesWithOrigin,
            ))
            .id();

        model_entities.insert(custom_model.id, entity);
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
        if let Some(entity) = model_entities.get(&asset_update.id) {
            let (mut scene, mut animation_graph, mut model) = model_query.get_mut(*entity).unwrap();

            let Some(model_config) = models.get_config(&asset_update.asset) else {
                net.disconnect(format!(
                    "Server sent model asset id that doesn't exist, id: {}",
                    asset_update.id
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
        if let Some(entity) = model_entities.get(&transform_update.id) {
            // TODO: I think this should be bug, server should not send model same tick it sends
            // transform updated. But there is 1-frame delay for model entity spawn for command
            // application. Should be disconnect I think, if bevy ever gets immediate command
            // application.
            let mut interpolation = match model_query.get_mut(*entity) {
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

fn interpolate_to_new_transform(
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
    mut model_query: Query<(&mut Model, &mut AnimationPlayer), With<AnimationGraphHandle>>,
    mut animation_events: EventReader<messages::ModelPlayAnimation>,
) {
    for animation in animation_events.read() {
        let Some(model_entity) = model_entities.get(&animation.model_id) else {
            // net.disconnect(
            //     "The server tried to play an animation for an entity that doesn't exist.",
            // );
            return;
        };

        let (model, mut animation_player) = model_query.get_mut(*model_entity).unwrap();

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

        animation_player.stop_all();
        animation_player.play(*animation_index);
        let active_animation = animation_player.animation_mut(*animation_index).unwrap();
        //dbg!(&active_animation);

        // When the server wants an animation to stop, it sends the same animation but with
        // repeat=false. Then we complete the current animation cycle and stop.
        if animation.repeat {
            active_animation.set_repeat(RepeatAnimation::Forever);
        } else {
            // TODO: It messes up the last frame
            // https://github.com/bevyengine/bevy/issues/10832
            let count = active_animation.completions() + 1;
            active_animation.set_repeat(RepeatAnimation::Count(count));
        }
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
