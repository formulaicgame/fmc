use std::time::Duration;

use bevy::{
    gltf::{Gltf, GltfMesh},
    math::Vec3A,
    prelude::*,
    render::{mesh::VertexAttributeValues, primitives::Aabb},
    window::{CursorGrabMode, PrimaryWindow},
};
use fmc_networking::{messages, NetworkClient, NetworkData};

use crate::{
    assets::models::Models,
    game_state::GameState,
    player::{PlayerCameraMarker, PlayerState},
    utils,
    world::{blocks::BlockFace, world_map::WorldMap, Origin},
};

use super::server::{
    items::{ItemBox, ItemBoxSection, Items, SelectedItemBox},
    InterfaceNode,
};

pub struct HandPlugin;
impl Plugin for HandPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(SwitchAnimation::default())
            .add_systems(PostStartup, setup)
            .add_systems(
                Update,
                (
                    equip_item,
                    play_switch_animation,
                    play_use_animation,
                    place_block,
                    send_clicks,
                )
                    .run_if(in_state(GameState::Playing)),
            );
    }
}

fn setup(mut commands: Commands, player_camera: Query<Entity, Added<PlayerCameraMarker>>) {
    if let Ok(entity) = player_camera.get_single() {
        commands.entity(entity).with_children(|parent| {
            parent.spawn(HandBundle::default());
        });
    }
}

#[derive(Bundle, Default)]
struct HandBundle {
    scene: SceneBundle,
    animation_player: AnimationPlayer,
    marker: HandMarker,
}

#[derive(Component)]
struct EquippedItem;

#[derive(Component, Default)]
struct HandMarker;

#[derive(Resource, Default)]
struct SwitchAnimation {
    elapsed: f32,
    // Transform we are going from
    old_transform: Transform,
    // How far down the old transform must be shifted down to not be visible anymore.
    old_offset: f32,
    // Transform we are going to
    new_transform: Transform,
    // Same but reverse
    new_offset: f32,
    // New scene that should be shown after old item has been hidden
    scene_handle: Handle<Scene>,
}
// Equips the item that is selected in any visible interface where equipment=true in the config.
// There should only ever be one such interface visible, if there are more, it will equip one at
// random.
fn equip_item(
    mut commands: Commands,
    net: Res<NetworkClient>,
    items: Res<Items>,
    gltf_assets: Res<Assets<Gltf>>,
    gltf_meshes: Res<Assets<GltfMesh>>,
    meshes: Res<Assets<Mesh>>,
    animation_clips: Res<Assets<AnimationClip>>,
    mut switch_animation: ResMut<SwitchAnimation>,
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
    hand_scene_query: Query<(Entity, &Handle<Scene>), With<HandMarker>>,
) {
    // equip and unequip when the equipment interface is hidden/shown or the selected box changes
    for (interface_node, item_box_section, selected) in changed_interface_query.iter() {
        if !item_box_section.is_equipment {
            continue;
        }

        if let Ok(entity) = equipped_entity_query.get_single() {
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
        let (hand_entity, hand_scene) = hand_scene_query.single();

        switch_animation.old_transform = switch_animation.new_transform;
        switch_animation.old_offset = switch_animation.new_offset;

        let mut new_transform = Transform::default();

        if let Some(item_id) = item_box.item_stack.item {
            let item = items.get(&item_id);
            let gltf = gltf_assets.get(&item.model_handle).unwrap();

            // This prevents triggering the switch animation when switching
            // between the same items. The server also sends a full interface update anytime an item is
            // picked up that is caught by this.
            if gltf.scenes[0] == *hand_scene {
                continue;
            }

            // In order for animation players to work, the entity it is part of needs to share
            // name with the AnimationClip paths. There is an animation player inserted deep in
            // the hierarchy below the hand entity. It is too cumbersome to get to. This is a hack.
            let name = Name::new(
                gltf.named_nodes
                    .keys()
                    .next()
                    .unwrap_or(&"model".to_owned())
                    .to_owned(),
            );
            commands
                .entity(hand_entity)
                .insert((name, AnimationPlayer::default()));

            let gltf_mesh = gltf_meshes.get(&gltf.meshes[0]).unwrap();
            // Extract aabb height from gltf in an error prone way. I don't know how
            // to do it through the scenes.
            let mut min: f32 = 0.0;
            let mut max: f32 = 0.0;
            for primitive in gltf_mesh.primitives.iter() {
                let mesh = meshes.get(&primitive.mesh).unwrap();
                let Some(VertexAttributeValues::Float32x3(vertices)) =
                    mesh.attribute(Mesh::ATTRIBUTE_POSITION)
                else {
                    continue;
                };
                for vertex in vertices.iter() {
                    min = min.min(vertex[1]);
                    max = max.max(vertex[1]);
                }
            }
            let height = max - min;

            let animation_handle = gltf.named_animations.get("left_click").unwrap().clone();
            let animation_clip = animation_clips.get(&animation_handle).unwrap();

            for curve in &animation_clip.curves()[0] {
                match &curve.keyframes {
                    Keyframes::Scale(frames) => {
                        new_transform.scale = *frames.last().unwrap();
                    }
                    Keyframes::Translation(frames) => {
                        new_transform.translation = *frames.last().unwrap();
                    }
                    Keyframes::Rotation(frames) => {
                        new_transform.rotation = *frames.last().unwrap();
                    }
                    _ => continue,
                }
            }

            switch_animation.new_transform = new_transform;
            switch_animation.new_offset = height;
            switch_animation.scene_handle = gltf.scenes[0].clone();
            switch_animation.elapsed = 0.0;
        } else {
            switch_animation.scene_handle = Handle::default();
            switch_animation.elapsed = 0.0;
        }
    }
}

// TODO: See how bevy does animation and find how to remove this 'finished' variable.
fn play_switch_animation(
    time: Res<Time>,
    mut switch_animation: ResMut<SwitchAnimation>,
    mut hand_query: Query<(&mut Transform, &mut Handle<Scene>), With<HandMarker>>,
    mut finished: Local<bool>,
) {
    const DURATION: f32 = 0.3;

    let (mut transform, mut scene) = hand_query.single_mut();

    if switch_animation.elapsed < DURATION / 2.0 {
        if switch_animation.elapsed + time.delta_seconds() > DURATION / 2.0 {
            let mut new_transform = switch_animation.new_transform;
            new_transform.translation.y -=
                (DURATION - switch_animation.elapsed) * switch_animation.new_offset;
            *transform = new_transform;
            *scene = switch_animation.scene_handle.clone();
        } else {
            // Lower equipped item below view, and switch scene handle
            let mut new_transform = switch_animation.old_transform;
            new_transform.translation.y -= switch_animation.elapsed * switch_animation.old_offset;
            *transform = new_transform;
        }

        *finished = false;
    } else if switch_animation.elapsed <= DURATION {
        let mut new_transform = switch_animation.new_transform;
        new_transform.translation.y -=
            (DURATION - switch_animation.elapsed) * switch_animation.new_offset;
        *transform = new_transform;

        *finished = false;
    } else if !*finished && switch_animation.new_transform != *transform {
        // elapsed is almost never exactly equal DURATION, so set it manually
        *transform = switch_animation.new_transform;
        *finished = true;
    }

    switch_animation.elapsed += time.delta_seconds();
}

fn play_use_animation(
    items: Res<Items>,
    gltf_assets: Res<Assets<Gltf>>,
    animation_clips: Res<Assets<AnimationClip>>,
    window: Query<&Window, With<PrimaryWindow>>,
    mouse_button_input: Res<ButtonInput<MouseButton>>,
    mut hand_animation_query: Query<(&mut Transform, &mut AnimationPlayer), With<HandMarker>>,
    equipped_item_query: Query<&ItemBox, With<EquippedItem>>,
) {
    let Ok(equipped_item) = equipped_item_query.get_single() else {
        return;
    };

    // TODO: Needs a robust way to see if interface is open
    // Only play if not in interface
    if window.single().cursor.visible {
        return;
    }

    let item = if let Some(item_id) = &equipped_item.item_stack.item {
        items.get(item_id)
    } else {
        return;
    };

    let gltf = gltf_assets.get(&item.model_handle).unwrap();
    let (mut transform, mut animation_player) = hand_animation_query.single_mut();

    let animation_handle = gltf.named_animations.get("left_click").unwrap();
    let animation_clip = animation_clips.get(animation_handle).unwrap();

    if mouse_button_input.pressed(MouseButton::Left) {
        if mouse_button_input.just_pressed(MouseButton::Left)
            || animation_player.elapsed() >= animation_clip.duration()
        {
            animation_player.start(animation_handle.clone());
        }
    } else if mouse_button_input.just_pressed(MouseButton::Right) {
        animation_player.start_with_transition(animation_handle.clone(), Duration::from_millis(10));
    } else if animation_player.is_finished() {
        // TODO: It messes up the last frame
        // https://github.com/bevyengine/bevy/issues/10832
        let mut new_transform = Transform::default();
        for curve in &animation_clip.curves()[0] {
            match &curve.keyframes {
                Keyframes::Scale(frames) => {
                    new_transform.scale = *frames.last().unwrap();
                }
                Keyframes::Translation(frames) => {
                    new_transform.translation = *frames.last().unwrap();
                }
                Keyframes::Rotation(frames) => {
                    new_transform.rotation = *frames.last().unwrap();
                }
                _ => continue,
            }
        }

        transform.set_if_neq(new_transform);
    }
}

fn send_clicks(
    window: Query<&Window, With<PrimaryWindow>>,
    mouse_button_input: Res<ButtonInput<MouseButton>>,
    net: Res<NetworkClient>,
) {
    if window.single().cursor.grab_mode != CursorGrabMode::None {
        if mouse_button_input.pressed(MouseButton::Left) {
            net.send_message(messages::LeftClick);
        } else if mouse_button_input.just_pressed(MouseButton::Right) {
            net.send_message(messages::RightClick);
        }
    }
}

// TODO: Needs repetition if button held down. Test to where it feels reasonably comfortable so
// that you can fly and place without having to pace yourself.
//
// Fakes a local block update to make it feel more responsive. The server will NOT know if it is
// a valid placement, so it will not correct it.
fn place_block(
    net: Res<NetworkClient>,
    world_map: Res<WorldMap>,
    items: Res<Items>,
    origin: Res<Origin>,
    mouse_button_input: Res<ButtonInput<MouseButton>>,
    mut equipped_query: Query<&mut ItemBox, With<EquippedItem>>,
    player_query: Query<(&Aabb, &GlobalTransform), With<PlayerState>>,
    camera_transform: Query<&GlobalTransform, With<PlayerCameraMarker>>,
    // We pretend the block update came from the server so it instantly updates without having to
    // rebound of the server.
    mut block_updates_events: EventWriter<NetworkData<messages::BlockUpdates>>,
) {
    if mouse_button_input.just_pressed(MouseButton::Right) {
        let (player_aabb, player_position) = player_query.single();
        let camera_transform = camera_transform.single();
        let Ok(mut equipped_item) = equipped_query.get_single_mut() else {
            return;
        };

        let (mut block_position, _block_id, block_face) = match world_map.raycast_to_block(
            &camera_transform.compute_transform(),
            origin.0,
            5.0,
        ) {
            Some(i) => i,
            None => return,
        };

        match block_face {
            BlockFace::Top => block_position.y += 1,
            BlockFace::Bottom => block_position.y -= 1,
            BlockFace::Front => block_position.z += 1,
            BlockFace::Back => block_position.z -= 1,
            BlockFace::Right => block_position.x += 1,
            BlockFace::Left => block_position.x -= 1,
        }

        let block_aabb = Aabb::from_min_max(
            (block_position - origin.0).as_vec3(),
            (block_position + 1 - origin.0).as_vec3(),
        );

        // TODO: This is too strict, you can't place blocks directly beneath / adjacently when
        // standing on an edge.
        let overlap = player_aabb.half_extents + block_aabb.half_extents
            - (player_aabb.center + player_position.translation_vec3a() - block_aabb.center).abs();

        if overlap.cmpgt(Vec3A::ZERO).all() {
            return;
        }

        let block_id = match equipped_item.item_stack.item {
            Some(item_id) => match &items.get(&item_id).block {
                Some(block_id) => *block_id,
                None => return,
            },
            None => return,
        };

        equipped_item.item_stack.subtract(1);

        let (chunk_position, block_index) =
            utils::world_position_to_chunk_position_and_block_index(block_position);
        let message = messages::BlockUpdates {
            chunk_position,
            blocks: vec![(block_index, block_id, None)],
        };

        // Pretend we get the block from the server so it gets the update immediately for mesh
        // generation. Makes it more responsive.
        block_updates_events.send(NetworkData::new(net.connection_id(), message));
    }
}
