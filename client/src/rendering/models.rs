use std::{collections::HashMap, ops::Deref};

use bevy::{
    animation::RepeatAnimation,
    gltf::Gltf,
    math::DVec3,
    pbr::NotShadowCaster,
    prelude::*,
    render::{mesh::Indices, primitives::Aabb, render_asset::RenderAssetUsages},
};
use fmc_networking::{messages, NetworkClient, NetworkData};

use crate::{
    assets::models::{GltfAnimationPlayers, Model, Models},
    game_state::GameState,
    world::{MovesWithOrigin, Origin},
};

pub struct ModelPlugin;
impl Plugin for ModelPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ModelEntities::default()).add_systems(
            Update,
            (
                handle_model_add_delete,
                update_model_asset,
                render_aabb,
                handle_transform_updates,
                interpolate_to_new_transform,
                play_animations.after(handle_model_add_delete),
                play_queued_animations,
            )
                .run_if(GameState::in_game),
        );
    }
}

/// Map from server model id to entity
#[derive(Resource, Deref, DerefMut, Default)]
struct ModelEntities(HashMap<u32, Entity>);

fn handle_model_add_delete(
    mut commands: Commands,
    origin: Res<Origin>,
    models: Res<Models>,
    gltf_assets: Res<Assets<Gltf>>,
    mut model_entities: ResMut<ModelEntities>,
    mut deleted_models: EventReader<NetworkData<messages::DeleteModel>>,
    mut new_models: EventReader<NetworkData<messages::NewModel>>,
) {
    for model in deleted_models.read() {
        if let Some(entity) = model_entities.remove(&model.id) {
            // BUG: Every time the model's scene handle changes, a new child entity is attached to
            // this entity. Presumably for the gltf meshes etc. These are not cleaned up when the
            // scene changes. When we call despawn_recursive here it complains about the child
            // entites not existing. Something to do with the hierarchy propagation probably? The
            // gltf stuff is deleted, but a reference to the entity is left hanging in the
            // children.
            warn!("The following warnings (if any) are the result of a bug.");
            commands.entity(entity).despawn_recursive();
        }
    }

    for new_model in new_models.read() {
        let model = if let Some(model) = models.get(&new_model.asset) {
            model
        } else {
            // Disconnect
            todo!();
        };

        // Server may send same id with intent to replace without deleting first
        if let Some(old_entity) = model_entities.remove(&new_model.id) {
            commands.entity(old_entity).despawn_recursive();
        }

        let gltf = gltf_assets.get(&model.handle).unwrap();

        let entity = commands
            .spawn(SceneBundle {
                scene: gltf.scenes[0].clone(),
                transform: Transform {
                    translation: (new_model.position - origin.as_dvec3()).as_vec3(),
                    rotation: new_model.rotation,
                    scale: new_model.scale,
                },
                ..default()
            })
            .insert(model.clone())
            .insert(QueuedAnimations::default())
            .insert(TransformInterpolation::default())
            .insert(MovesWithOrigin)
            .id();

        model_entities.insert(new_model.id, entity);
    }
}

fn update_model_asset(
    model_entities: Res<ModelEntities>,
    models: Res<Models>,
    gltf_assets: Res<Assets<Gltf>>,
    mut asset_updates: EventReader<NetworkData<messages::ModelUpdateAsset>>,
    mut model_query: Query<&mut Handle<Scene>, With<Model>>,
) {
    for asset_update in asset_updates.read() {
        if let Some(entity) = model_entities.get(&asset_update.id) {
            let mut scene_handle = model_query.get_mut(*entity).unwrap();

            *scene_handle = if let Some(model) = models.get(&asset_update.asset) {
                gltf_assets.get(&model.handle).unwrap().scenes[0].clone()
            } else {
                // Disconnect?
                todo!();
            };
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
    mut transform_updates: EventReader<NetworkData<messages::ModelUpdateTransform>>,
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

// Animations will often arrive at the same tick as the model. It takes some time to load the
// scene, so we have to store the animations until the model is ready.
#[derive(Component, DerefMut, Deref, Default)]
struct QueuedAnimations(Vec<messages::ModelPlayAnimation>);

fn play_animations(
    net: Res<NetworkClient>,
    gltf_assets: Res<Assets<Gltf>>,
    model_entities: Res<ModelEntities>,
    mut model_query: Query<(&Model, Option<&GltfAnimationPlayers>, &mut QueuedAnimations)>,
    mut animation_players: Query<&mut AnimationPlayer>,
    mut animation_events: EventReader<NetworkData<messages::ModelPlayAnimation>>,
) {
    for animation in animation_events.read() {
        let Some(model_entity) = model_entities.get(&animation.model_id) else {
            net.disconnect(
                "The server tried to play an animation for an entity that doesn't exist.",
            );
            return;
        };

        let (model, maybe_animation_players, mut queued_animations) =
            model_query.get_mut(*model_entity).unwrap();

        let Some(gltf_animation_players) = maybe_animation_players else {
            queued_animations.push(animation.deref().clone());
            continue;
        };

        let gltf = gltf_assets.get(&model.handle).unwrap();

        let Some(animation_handle) = gltf.animations.get(animation.animation_index as usize) else {
            // TODO: Need to print the name of the model in the error message for debugging.
            net.disconnect(format!(
                "The server sent an animation that doesn't exist. Animation index was '{}'",
                animation.animation_index
            ));
            return;
        };

        let mut animation_player = animation_players
            .get_mut(gltf_animation_players.main.unwrap())
            .unwrap();
        animation_player.play(animation_handle.clone());

        // When the server wants an animation to stop, it sends the same animation but with
        // repeat=false. Then we complete the current animation cycle and stop.
        if animation.repeat {
            animation_player.set_repeat(RepeatAnimation::Forever);
        } else {
            // TODO: It messes up the last frame
            // https://github.com/bevyengine/bevy/issues/10832
            let count = animation_player.completions() + 1;
            animation_player.set_repeat(RepeatAnimation::Count(count));
        }
    }
}

fn play_queued_animations(
    net: Res<NetworkClient>,
    gltf_assets: Res<Assets<Gltf>>,
    mut model_query: Query<
        (&Model, &GltfAnimationPlayers, &mut QueuedAnimations),
        Added<GltfAnimationPlayers>,
    >,
    mut animation_players: Query<&mut AnimationPlayer>,
) {
    for (model, gltf_animation_players, mut queued_animations) in model_query.iter_mut() {
        let gltf = gltf_assets.get(&model.handle).unwrap();

        for animation in queued_animations.drain(..).rev() {
            let Some(animation_handle) = gltf.animations.get(animation.animation_index as usize)
            else {
                // TODO: Need to print the name of the model in the error message for debugging.
                net.disconnect(format!(
                    "The server sent an animation that doesn't exist. Animation index was '{}'",
                    animation.animation_index
                ));
                return;
            };

            let mut animation_player = animation_players
                .get_mut(gltf_animation_players.main.unwrap())
                .unwrap();
            animation_player.play(animation_handle.clone());

            if animation.repeat {
                animation_player.set_repeat(RepeatAnimation::Forever);
            } else {
                animation_player.set_repeat(RepeatAnimation::Never);
            }
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
                PbrBundle {
                    mesh: meshes.add(mesh),
                    material: materials.add(StandardMaterial {
                        base_color: Color::rgb(0.0, 1.0, 0.0),
                        unlit: true,
                        ..default()
                    }),
                    transform: Transform {
                        scale: 1.0 / transform.scale,
                        translation: Vec3::new(0.0, 0.0, 0.0),
                        ..default()
                    },
                    ..default()
                },
                NotShadowCaster,
            ))
            .id();
        commands.entity(entity).add_child(child);
    }
}
