use bevy::{
    animation::AnimationTarget,
    gltf::Gltf,
    prelude::*,
    render::view::RenderLayers,
    scene::SceneInstanceReady,
    window::{CursorGrabMode, PrimaryWindow},
};
use fmc_protocol::messages;

use crate::{
    assets::models::{ModelAssetId, Models},
    game_state::GameState,
    networking::NetworkClient,
    player::Head,
};

use super::server::{
    items::{ItemBox, ItemBoxSection, Items, SelectedItemBox},
    InterfaceNode,
};

pub struct HandPlugin;
impl Plugin for HandPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PostStartup, setup).add_systems(
            Update,
            (
                equip_item,
                play_equip_animation,
                play_use_animation,
                //place_block,
                send_clicks,
                // workarounds for https://github.com/bevyengine/bevy/issues/10832
                //mark_animated_entity,
                //set_correct_transform_after_animation_finished,
                remove_finished_animations
                    //.after(set_correct_transform_after_animation_finished)
                    .after(play_equip_animation)
                    .after(play_use_animation)
                    .after(equip_item),
            )
                .run_if(in_state(GameState::Playing)),
        );
    }
}

/// Currently [`RenderLayers`] are not applied to children of a scene.
/// This [`SceneInstanceReady`] observer applies the [`RenderLayers`]
/// of a [`SceneRoot`] to all children with a [`Transform`] and without a [`RenderLayers`].
///
/// See [#12461](https://github.com/bevyengine/bevy/issues/12461) for current status.
pub fn apply_render_layers(
    trigger: Trigger<SceneInstanceReady>,
    mut commands: Commands,
    children: Query<&Children>,
    transforms: Query<&Transform, Without<RenderLayers>>,
    query: Query<(Entity, &RenderLayers)>,
) {
    let Ok((parent, render_layers)) = query.get(trigger.target()) else {
        return;
    };
    children.iter_descendants(parent).for_each(|entity| {
        if transforms.contains(entity) {
            commands.entity(entity).insert(render_layers.clone());
        }
    });
}

fn setup(mut commands: Commands, player_camera: Query<Entity, Added<Head>>) {
    let camera_entity = player_camera.single().unwrap();
    commands.entity(camera_entity).with_children(|parent| {
        parent
            .spawn((
                Hand::default(),
                SceneRoot::default(),
                // This is linked to animation targets by the same system that does it for server models. The
                // animation graph must be added manually.
                AnimationPlayer::default(),
                AnimationTransitions::default(),
                AnimationGraphHandle::default(),
                RenderLayers::layer(1),
            ))
            .observe(apply_render_layers);
    });
}

#[derive(Component, Default)]
struct Hand {
    // True while an item is being equipped and unequipped
    in_equip_animation: bool,
    // While an item is equipped it is stored here
    equipped: Option<ModelAssetId>,
    // The model being unequipped is stored here while in the animation
    being_unequipped: Option<ModelAssetId>,
}

#[derive(Component)]
struct EquippedItem;

fn equip_item(
    mut commands: Commands,
    net: Res<NetworkClient>,
    items: Res<Items>,
    models: Res<Models>,
    changed_interface_query: Query<
        (&InterfaceNode, &ItemBoxSection, &SelectedItemBox),
        Changed<SelectedItemBox>,
    >,
    item_box_query: Query<&ItemBox>,
    equipped_entity_query: Query<Entity, With<EquippedItem>>,
    changed_equipped_item_query: Query<
        &ItemBox,
        (
            Or<(Changed<ItemBox>, Added<EquippedItem>)>,
            With<EquippedItem>,
        ),
    >,
    mut hand_query: Query<(&mut Hand, &mut AnimationPlayer)>,
) {
    // equip and unequip when the equipment interface is hidden/shown or the selected box changes
    for (interface_node, item_box_section, selected) in changed_interface_query.iter() {
        if !item_box_section.is_equipment {
            continue;
        }

        if let Ok(entity) = equipped_entity_query.single() {
            commands.entity(entity).remove::<EquippedItem>();
        }

        let item_box = item_box_query.get(selected.0).unwrap();
        net.send_message(messages::InterfaceEquipItem {
            interface_path: interface_node.path.to_owned(),
            index: item_box.index as u32,
        });

        commands.entity(selected.0).insert(EquippedItem);
    }

    // equip new item when the selected item changes.
    for item_box in changed_equipped_item_query.iter() {
        let (mut hand, mut animation_player) = hand_query.single_mut().unwrap();

        if let Some(item_id) = item_box.item_stack.item() {
            let item = items.get(&item_id);

            // This prevents triggering the switch animation when switching between the same items.
            // The server also sends a full interface update anytime an item is picked up that is
            // caught by this.
            if hand.equipped == Some(item.equip_model) {
                // If switching between two stacks of the same item it should do nothing.
                continue;
            } else if hand.being_unequipped == Some(item.equip_model) {
                // If the item that was just equipped is the same as the one being unequipped, just
                // revert the animation.
                let model = models.get_config(&item.equip_model).unwrap();
                animation_player
                    .play(model.named_animations["equip"])
                    .set_speed(1.0);
                hand.being_unequipped = None;
            } else if hand.being_unequipped.is_none() {
                // Start unequipping the currently held item
                if let Some(equipped) = hand.equipped.take() {
                    let model = models.get_config(&equipped).unwrap();
                    animation_player
                        .play(model.named_animations["equip"])
                        .set_speed(-1.0)
                        // XXX: Bug, reverse animations instantly complete because of wrong conditinon
                        // check
                        .set_repeat(bevy::animation::RepeatAnimation::Count(2));
                    hand.being_unequipped = Some(equipped);
                }
            }

            hand.in_equip_animation = true;

            let model = models.get_config(&item.equip_model).unwrap();
            if !model.named_animations.contains_key("equip") {
                warn!("Missing equip animation, can't equip item");
                continue;
            };

            hand.equipped = Some(item.equip_model);
        } else {
            if hand.being_unequipped.is_none() {
                if let Some(equipped) = hand.equipped.take() {
                    let model = models.get_config(&equipped).unwrap();
                    animation_player
                        .play(model.named_animations["equip"])
                        .set_speed(-1.0)
                        // XXX: Bug, reverse animations instantly complete because of wrong conditinon
                        // check
                        .set_repeat(bevy::animation::RepeatAnimation::Count(2));
                    hand.equipped = None;
                    hand.being_unequipped = Some(equipped);
                    hand.in_equip_animation = true;
                }
            }
        }
    }
}

fn play_equip_animation(
    models: Res<Models>,
    gltfs: Res<Assets<Gltf>>,
    mut hand_query: Query<(
        &mut AnimationPlayer,
        &mut SceneRoot,
        &mut Hand,
        &mut AnimationGraphHandle,
        &mut Visibility,
    )>,
) {
    let (mut animation_player, mut scene_handle, mut hand, mut animation_graph, mut visibility) =
        hand_query.single_mut().unwrap();

    if !hand.in_equip_animation {
        return;
    }

    for (_index, active_animation) in animation_player.playing_animations() {
        if active_animation.speed() == -1.0 {
            if !active_animation.is_finished() {
                return;
            } else {
                // If there is no new model, we set it to nothing here so that the model is
                // despawned.
                *scene_handle = SceneRoot::default();
            }
        }
    }

    if hand.being_unequipped.take().is_some() {
        // Remove the animation from the player so it doesn't linger
        animation_player.stop_all();
    }

    if let Some(model_id) = &hand.equipped {
        let model = models.get_config(model_id).unwrap();
        let animation_index = model.named_animations["equip"];

        if !animation_player.is_playing_animation(animation_index) {
            // Need to remove unequip animation or it will linger. Bevy doesn't remove finished
            // animations.
            animation_player.stop_all();

            *visibility = Visibility::Hidden;

            let gltf = gltfs.get(&model.gltf_handle).unwrap();
            *scene_handle = SceneRoot(gltf.scenes[0].clone());
            *animation_graph = AnimationGraphHandle(model.animation_graph.clone().unwrap());
        } else {
            // TODO: This is a hack. If you let the model stay visible while it spawns it will show a single frame
            // of the model without the animation applied. This is probably because of the delay
            // caused by having to transfer the animation targets to the animation player.
            *visibility = Visibility::Visible;
        }

        let animation = animation_player.play(animation_index);

        if !animation.is_finished() {
            return;
        }
    }

    hand.in_equip_animation = false;
}

// #[derive(Component)]
// struct AnimatedMarker;
//
// fn mark_animated_entity(
//     mut commands: Commands,
//     models: Res<Models>,
//     animation_graphs: Res<Assets<AnimationGraph>>,
//     animation_clips: Res<Assets<AnimationClip>>,
//     hand_entity: Query<(&Hand, &AnimationGraphHandle), Added<Children>>,
//     animation_targets: Query<(Entity, &AnimationTarget)>,
// ) {
//     let Ok((hand, animation_graph)) = hand_entity.get_single() else {
//         return;
//     };
//
//     let Some(model_config) = hand
//         .equipped
//         .and_then(|model_id| models.get_config(&model_id))
//     else {
//         return;
//     };
//
//     let Some(left_click_index) = model_config.named_animations.get("left_click") else {
//         return;
//     };
//
//     let animation_graph = animation_graphs.get(animation_graph).unwrap();
//     let left_click_node = animation_graph.get(*left_click_index).unwrap();
//     let AnimationNodeType::Clip(left_click_handle) = &left_click_node.node_type else {
//         unreachable!();
//     };
//     let left_click = animation_clips.get(left_click_handle).unwrap();
//
//     let Some(target_id) = left_click
//         .curves()
//         .iter()
//         .next()
//         .map(|(target, _curve)| target.clone())
//     else {
//         return;
//     };
//
//     for (entity, animation_target) in animation_targets.iter() {
//         if animation_target.id == target_id {
//             commands.entity(entity).insert(AnimatedMarker);
//             return;
//         }
//     }
// }

// fn set_correct_transform_after_animation_finished(
//     animation_graphs: Res<Assets<AnimationGraph>>,
//     animation_clips: Res<Assets<AnimationClip>>,
//     hand_query: Query<(&AnimationPlayer, &AnimationGraphHandle), With<Hand>>,
//     mut animated: Query<(&mut Transform, &AnimationTarget), With<AnimatedMarker>>,
// ) {
//     let Ok((mut transform, target)) = animated.get_single_mut() else {
//         return;
//     };
//
//     let (animation_player, graph_handle) = hand_query.single();
//     for (node_index, animation) in animation_player.playing_animations() {
//         if !animation.is_finished() || animation.speed() < 0.0 {
//             continue;
//         }
//         let animation_graph = animation_graphs.get(graph_handle).unwrap();
//         let AnimationNodeType::Clip(animation_clip_handle) =
//             &animation_graph.get(*node_index).unwrap().node_type
//         else {
//             unreachable!()
//         };
//
//         let animation_clip = animation_clips.get(animation_clip_handle).unwrap();
//         let Some(curves) = animation_clip.curves_for_target(target.id) else {
//             continue;
//         };
//
//         for curve in curves {
//             match &curve.keyframes {
//                 Keyframes::Rotation(rotations) => {
//                     transform.rotation = rotations.last().cloned().unwrap();
//                 }
//                 Keyframes::Translation(translations) => {
//                     transform.translation = translations.last().cloned().unwrap();
//                 }
//                 Keyframes::Scale(scales) => {
//                     transform.scale = scales.last().cloned().unwrap();
//                 }
//                 _ => (),
//             }
//         }
//     }
// }

fn remove_finished_animations(mut animation_player: Query<&mut AnimationPlayer, With<Hand>>) {
    let mut animation_player = animation_player.single_mut().unwrap();
    let mut to_stop = Vec::new();
    for (index, animation) in animation_player.playing_animations() {
        if animation.is_finished() {
            to_stop.push(*index);
        }
    }

    for index in to_stop {
        animation_player.stop(index);
    }
}

fn play_use_animation(
    models: Res<Models>,
    animation_graphs: Res<Assets<AnimationGraph>>,
    animation_clips: Res<Assets<AnimationClip>>,
    window: Query<&Window, With<PrimaryWindow>>,
    mouse_button_input: Res<ButtonInput<MouseButton>>,
    mut hand_query: Query<(&mut AnimationPlayer, &Hand)>,
) {
    // TODO: Needs a robust way to see if interface is open
    //
    // Only play if not in interface
    if window.single().unwrap().cursor_options.visible {
        return;
    }

    let (mut animation_player, hand) = hand_query.single_mut().unwrap();

    let Some(model) = &hand.equipped else {
        return;
    };

    let model_config = models.get_config(model).unwrap();

    let Some(left_click) = model_config.named_animations.get("left_click").cloned() else {
        return;
    };

    if mouse_button_input.just_pressed(MouseButton::Left) {
        // TODO: Transition
        // Play from beginning even if in the middle of an animation.
        animation_player.stop(left_click);
        animation_player.start(left_click);
    } else if mouse_button_input.pressed(MouseButton::Left) {
        // Keep playing from current position if the mouse buttton is held
        let animation = animation_player.play(left_click);
        let animation_graph = animation_graphs
            .get(model_config.animation_graph.as_ref().unwrap())
            .unwrap();
        let AnimationNodeType::Clip(clip_handle) =
            &animation_graph.get(left_click).as_ref().unwrap().node_type
        else {
            unreachable!()
        };
        let clip = animation_clips.get(clip_handle).unwrap();

        if animation.elapsed() / clip.duration() > 0.65 {
            animation.replay();
        }
    } else if mouse_button_input.just_pressed(MouseButton::Right) {
        animation_player.stop(left_click);
        animation_player.start(left_click);
    }
}

fn send_clicks(
    net: Res<NetworkClient>,
    window: Query<&Window, With<PrimaryWindow>>,
    mouse_button_input: Res<ButtonInput<MouseButton>>,
) {
    if window.single().unwrap().cursor_options.grab_mode != CursorGrabMode::None {
        if mouse_button_input.pressed(MouseButton::Left) {
            net.send_message(messages::LeftClick::Press);
        } else if mouse_button_input.just_released(MouseButton::Left) {
            net.send_message(messages::LeftClick::Release);
        } else if mouse_button_input.just_pressed(MouseButton::Right) {
            net.send_message(messages::RightClick::Press);
        }
        // TODO: Full right clicks don't work for interfaces. If you open an interface on the down
        // press the cursor won't be locked anymore, so it won't send the release event.
        // else if mouse_button_input.just_released(MouseButton::Right) {
        //     dbg!("send release");
        //     net.send_message(messages::RightClick::Release);
        // }
    }
}

// TODO: Needs repetition if button held down. Test to where it feels reasonably comfortable so
// that you can fly and place without having to pace yourself.
//
// Fakes a local block update to make it feel more responsive. The server will NOT know if it is
// a valid placement, so it will not correct it.
// fn place_block(
//     world_map: Res<WorldMap>,
//     items: Res<Items>,
//     origin: Res<Origin>,
//     mouse_button_input: Res<ButtonInput<MouseButton>>,
//     mut equipped_query: Query<&mut ItemBox, With<EquippedItem>>,
//     player_query: Query<(&Aabb, &GlobalTransform), With<PlayerState>>,
//     camera_transform: Query<&GlobalTransform, With<PlayerCameraMarker>>,
//     // We pretend the block update came from the server so it instantly updates without having to
//     // rebound of the server.
//     mut block_updates_events: EventWriter<messages::BlockUpdates>,
// ) {
//     if mouse_button_input.just_pressed(MouseButton::Right) {
//         let (player_aabb, player_position) = player_query.single();
//         let camera_transform = camera_transform.single();
//         let Ok(mut equipped_item) = equipped_query.get_single_mut() else {
//             return;
//         };
//
//         let (mut block_position, _block_id, block_face) = match world_map.raycast_to_block(
//             &camera_transform.compute_transform(),
//             origin.0,
//             5.0,
//         ) {
//             Some(i) => i,
//             None => return,
//         };
//
//         match block_face {
//             BlockFace::Top => block_position.y += 1,
//             BlockFace::Bottom => block_position.y -= 1,
//             BlockFace::Front => block_position.z += 1,
//             BlockFace::Back => block_position.z -= 1,
//             BlockFace::Right => block_position.x += 1,
//             BlockFace::Left => block_position.x -= 1,
//         }
//
//         let block_aabb = Aabb::from_min_max(
//             (block_position - origin.0).as_vec3(),
//             (block_position + 1 - origin.0).as_vec3(),
//         );
//
//         // TODO: This is too strict, you can't place blocks directly beneath / adjacently when
//         // standing on an edge.
//         let player_overlap = player_aabb.half_extents + block_aabb.half_extents
//             - (player_aabb.center + player_position.translation_vec3a() - block_aabb.center).abs();
//
//         if player_overlap.cmpgt(Vec3A::ZERO).all() {
//             return;
//         }
//
//         let block_id = match equipped_item.item_stack.item {
//             Some(item_id) => match &items.get(&item_id).block {
//                 Some(block_id) => *block_id,
//                 None => return,
//             },
//             None => return,
//         };
//
//         let block_state = if Blocks::get().get_config(block_id).can_have_block_state() {
//             if block_face != BlockFace::Bottom && block_face != BlockFace::Top {
//                 Some(BlockState::new(block_face.to_rotation()))
//             } else {
//                 None
//             }
//         } else {
//             None
//         };
//
//         equipped_item.item_stack.subtract(1);
//
//         let (chunk_position, block_index) =
//             utils::world_position_to_chunk_position_and_block_index(block_position);
//         let message = messages::BlockUpdates {
//             chunk_position,
//             blocks: vec![(block_index, block_id, block_state.map(|s| s.0))],
//         };
//
//         // Pretend we get the block from the server so it gets the update immediately for mesh
//         // generation. Makes it more responsive.
//         block_updates_events.send(message);
//     }
// }
