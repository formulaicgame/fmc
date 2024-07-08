use bevy::{
    input::mouse::MouseMotion,
    prelude::*,
    window::{CursorGrabMode, PrimaryWindow},
};

use fmc_networking::{messages, NetworkClient, NetworkData};

use crate::{
    game_state::GameState,
    settings::Settings,
    world::{
        blocks::Blocks,
        world_map::{chunk::Chunk, WorldMap},
        Origin,
    },
};

pub struct CameraPlugin;
impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                (
                    rotate_camera,
                    fog,
                    handle_camera_rotation_from_server,
                    handle_camera_position_from_server,
                )
                    .run_if(GameState::in_game),
                update_render_distance.run_if(resource_changed::<Settings>),
            ),
        );
    }
}

#[derive(Bundle)]
pub struct CameraBundle {
    camera_3d: Camera3dBundle,
    marker: PlayerCameraMarker,
    // XXX: Remove in future if requirement for parent to have it is removed. Needed for
    // equipped item
    visibility: VisibilityBundle,
}

impl Default for CameraBundle {
    fn default() -> Self {
        Self {
            camera_3d: Camera3dBundle {
                projection: PerspectiveProjection {
                    fov: std::f32::consts::PI / 3.0,
                    ..default()
                }
                .into(),
                ..default()
            },
            marker: PlayerCameraMarker::default(),
            visibility: VisibilityBundle::default(),
        }
    }
}

#[derive(Component, Default)]
pub struct PlayerCameraMarker;

fn update_render_distance(
    settings: Res<Settings>,
    mut projection_query: Query<&mut Projection, With<PlayerCameraMarker>>,
) {
    // TODO: This is Mut<Projection> so it complains, idk how to do it properly
    let mut projection = projection_query.single_mut();
    let perspective_projection = match &mut *projection {
        Projection::Perspective(p) => p,
        _ => unreachable!(),
    };

    let new_far = settings.render_distance as f32 * Chunk::SIZE as f32;
    if new_far != perspective_projection.far {
        perspective_projection.far = new_far;
    }
}

/// Handles looking around if cursor is locked
fn rotate_camera(
    window: Query<&Window, With<PrimaryWindow>>,
    settings: Res<Settings>,
    net: Res<NetworkClient>,
    mut mouse_events: EventReader<MouseMotion>,
    mut camera_query: Query<&mut Transform, With<Camera>>,
) {
    let window = window.single();

    // Mouse in use by some interface
    if window.cursor.visible == true {
        return;
    }

    // It empties the iterator so it can't access it after loop.
    let should_send = mouse_events.len() > 0;

    for ev in mouse_events.read() {
        let mut transform = camera_query.single_mut();

        if window.cursor.grab_mode != CursorGrabMode::Locked {
            return;
        }

        let (mut yaw, mut pitch, _) = transform.rotation.to_euler(EulerRot::YXZ);
        yaw -= (settings.sensitivity * ev.delta.x * window.width()).to_radians();
        pitch -= (settings.sensitivity * ev.delta.y * window.height()).to_radians();
        pitch = pitch.clamp(-1.54, 1.54);

        transform.rotation =
            Quat::from_axis_angle(Vec3::Y, yaw) * Quat::from_axis_angle(Vec3::X, pitch);
    }

    if should_send {
        net.send_message(messages::PlayerCameraRotation {
            rotation: camera_query.single().rotation,
        })
    }
}

// Forced camera rotation by the server.
fn handle_camera_rotation_from_server(
    mut camera_rotation_events: EventReader<NetworkData<messages::PlayerCameraRotation>>,
    mut camera_q: Query<&mut Transform, With<Camera>>,
) {
    for rotation_event in camera_rotation_events.read() {
        let mut transform = camera_q.single_mut();
        transform.rotation = rotation_event.rotation;
    }
}

// Forced camera position by the server
fn handle_camera_position_from_server(
    mut camera_position_events: EventReader<NetworkData<messages::PlayerCameraPosition>>,
    mut camera_q: Query<&mut Transform, With<Camera>>,
) {
    for position_event in camera_position_events.read() {
        let mut transform = camera_q.single_mut();
        transform.translation = position_event.position;
    }
}

fn fog(
    settings: Res<Settings>,
    origin: Res<Origin>,
    mut camera_transform_query: Query<
        (&GlobalTransform, &Projection, &mut FogSettings),
        (With<PlayerCameraMarker>, Changed<GlobalTransform>),
    >,
    world_map: Res<WorldMap>,
) {
    for (transform, projection, mut fog_settings) in camera_transform_query.iter_mut() {
        let (angle, near) = match projection {
            Projection::Perspective(projection) => (projection.fov, projection.near),
            _ => unreachable!(),
        };

        // TODO: Without this if you peek above water it will still have the water fog until the
        // camera origin comes up farther. With this if you peek under water it will not render the
        // fog until the top of the camera is sumberged. I would like to not need this tradeoff,
        // some kind of split.
        //
        // Only render fog when the camera is completely immersed in the block.
        let mut camera_frustum_near_top = transform.translation() + transform.forward() * near;
        // TODO: This angle division of 1.5 should technically be 2.0 no? If the angle is the
        // vertical fov, you want half. This yielded incorrect results though, 1.5 is better, but
        // still wrong.
        camera_frustum_near_top.y += near / (angle / 1.5).cos();

        let camera_top_position = (camera_frustum_near_top).as_ivec3() + origin.0;
        let Some(block_id) = world_map.get_block(&camera_top_position) else {
            continue;
        };

        let blocks = Blocks::get();
        let block_config = blocks.get_config(block_id);
        if let Some(fog) = block_config.fog_settings() {
            *fog_settings = fog.clone();
        } else {
            *fog_settings = settings.fog.clone();
        }
    }
}

// TODO: Left unfinished, doesn't render outline.
// Target the block the player is looking at.
//fn outline_selected_block(
//    world_map: Res<WorldMap>,
//    camera_query: Query<&GlobalTransform, (With<Camera>, Changed<GlobalTransform>)>,
//) {
//    let camera_transform = camera_query.single();
//
//    // We need to find the first block the ray intersects with, it is then marked as the origin.
//    // From this point we can jump from one block to another easily.
//    let forward = camera_transform.forward();
//    let direction = forward.signum();
//
//    // How far along the forward vector you need to go to hit the next block in each direction.
//    // This makes more sense if you mentally align it with the block grid.
//    //
//    // Also this relies on some peculiar behaviour where normally f32.fract() would retain the sign
//    // of the fraction, vec3.fract() instead does self - self.floor(). This results in having the
//    // correct value for the negative direction, but it has to be flipped for the positive
//    // direction, which is the vec3::select.
//    let mut distance_next = camera_transform.translation.fract();
//    distance_next = Vec3::select(
//        direction.cmpeq(Vec3::ONE),
//        1.0 - distance_next,
//        distance_next,
//    );
//    distance_next = distance_next / forward.abs();
//
//    // How far along the forward vector you need to go to traverse one block in each direction.
//    let t_block = 1.0 / forward.abs();
//    // +/-1 to shift block_pos when it hits the grid
//    let step = direction.as_ivec3();
//
//    let mut block_pos = camera_transform.translation.floor().as_ivec3();
//
//    for _ in 0..5 {
//        if distance_next.x < distance_next.y && distance_next.x < distance_next.z {
//            block_pos.x += step.x;
//            distance_next.x += t_block.x;
//
//            if let Some(block_id) = world_map.get_block(&block_pos) {
//                if block_id == 0 {
//                    continue;
//                }
//                looked_at.0 = Some((
//                    block_pos,
//                    if direction.x == 1.0 {
//                        BlockSide::Left
//                    } else {
//                        BlockSide::Right
//                    },
//                ));
//                return;
//            }
//        } else if distance_next.z < distance_next.x && distance_next.z < distance_next.y {
//            block_pos.z += step.z;
//            distance_next.z += t_block.z;
//
//            if let Some(block_id) = world_map.get_block(&block_pos) {
//                if block_id == 0 {
//                    continue;
//                }
//                looked_at.0 = Some((
//                    block_pos,
//                    if direction.z == 1.0 {
//                        BlockSide::Back
//                    } else {
//                        BlockSide::Front
//                    },
//                ));
//                return;
//            }
//        } else {
//            block_pos.y += step.y;
//            distance_next.y += t_block.y;
//
//            if let Some(block_id) = world_map.get_block(&block_pos) {
//                if block_id == 0 {
//                    continue;
//                }
//                looked_at.0 = Some((
//                    block_pos,
//                    if direction.y == 1.0 {
//                        BlockSide::Bottom
//                    } else {
//                        BlockSide::Top
//                    },
//                ));
//                return;
//            }
//        }
//    }
//
//    looked_at.0 = None;
//}
