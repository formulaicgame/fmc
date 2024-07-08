// TODO: This needs a lot of refinement. Bobbing while walking. Jumping feels floaty. Bobbing on
// the water is too sharp. Falling speed is too slow, but while jumping you fall too fast.

use bevy::{
    math::Vec3A,
    prelude::*,
    render::primitives::Aabb,
    window::{CursorGrabMode, PrimaryWindow},
};
use fmc_networking::{messages, NetworkClient, NetworkData};

use crate::{
    game_state::GameState,
    player::PlayerState,
    world::{
        blocks::{Blocks, Friction},
        world_map::WorldMap,
        Origin,
    },
};

// sqrt(2 * gravity * wanted height(1.4)) + some for air resistance
const JUMP_VELOCITY: f32 = 9.0;
const GRAVITY: Vec3 = Vec3::new(0.0, -32.0, 0.0);
// TODO: I think this should be a thing only if you hold space. If you are skilled you can press
// space again as soon as you land if you have released it in the meantime.
// TODO: It feels nice when you jump up a block, but when jumping down it does nothing, feels like
// bouncing. Maybe replace with a jump timer when you land so it's constant? I feel like it would
// be better if you could jump faster when jumping downwards, but not as much as now.
//
// This is needed so that whenever you land early you can't just instantly jump again.
// v_t = v_0 * at => (v_t - v_0) / a = t
const JUMP_TIME: f32 = JUMP_VELOCITY * 1.7 / -GRAVITY.y;

pub struct MovementPlugin;
impl Plugin for MovementPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, toggle_flight.run_if(in_state(GameState::Playing)))
            .add_systems(
                FixedUpdate,
                (
                    change_player_acceleration,
                    simulate_player_physics,
                    swimming,
                )
                    .chain()
                    .run_if(in_state(GameState::Playing)),
            )
            // TODO: This is another one of the things the server just sends on connection.
            // Workaround by just having it run all the time, but once the server can be notified
            // that the client is actually ready to receive it should be moved above with the rest.
            .add_systems(Update, handle_position_updates_from_server);
    }
}

#[derive(Deref)]
struct Timer {
    pub last: std::time::Instant,
}

impl Default for Timer {
    fn default() -> Self {
        return Self {
            last: std::time::Instant::now(),
        };
    }
}

fn handle_position_updates_from_server(
    origin: Res<Origin>,
    mut position_events: EventReader<NetworkData<messages::PlayerPosition>>,
    mut player_query: Query<&mut Transform, With<PlayerState>>,
) {
    for event in position_events.read() {
        let mut transform = player_query.single_mut();
        transform.translation = (event.position - origin.as_dvec3()).as_vec3();
    }
}

// TODO: Hack until proper input handling, note pressing fast three times will put you back into
// the original state.
fn toggle_flight(
    keys: Res<ButtonInput<KeyCode>>,
    window: Query<&Window, With<PrimaryWindow>>,
    mut query: Query<&mut PlayerState>,
    mut timer: Local<Timer>,
) {
    let window = window.single();
    if window.cursor.grab_mode == CursorGrabMode::None {
        return;
    }

    for key in keys.get_just_released() {
        if KeyCode::Space == *key {
            if std::time::Instant::now()
                .duration_since(timer.last)
                .as_millis()
                < 250
            {
                let mut player = query.single_mut();
                player.is_flying = !player.is_flying;
                player.velocity = Vec3::ZERO;
            } else {
                timer.last = std::time::Instant::now();
            }
        }
    }
}

// TODO: This blends moving and flying movement, they should be split in separate systems
/// Handles keyboard input and movement
fn change_player_acceleration(
    keys: Res<ButtonInput<KeyCode>>,
    window: Query<&Window, With<PrimaryWindow>>,
    mut player_query: Query<&mut PlayerState>,
    camera_query: Query<&Transform, With<Camera>>,
    mut last_jump: Local<Timer>,
) {
    let mut player = player_query.single_mut();
    let camera_transform = camera_query.single();

    let window = window.single();

    let camera_forward = camera_transform.forward();
    let forward = Vec3::new(camera_forward.x, 0., camera_forward.z);
    let sideways = Vec3::new(-camera_forward.z, 0., camera_forward.x);

    if player.is_flying {
        player.velocity.y = 0.0;
    }

    let mut horizontal_acceleration = Vec3::ZERO;
    let mut vertical_acceleration = Vec3::ZERO;
    for key in keys.get_pressed() {
        if window.cursor.grab_mode != CursorGrabMode::None {
            match key {
                KeyCode::KeyW => horizontal_acceleration += forward,
                KeyCode::KeyS => horizontal_acceleration -= forward,
                KeyCode::KeyA => horizontal_acceleration -= sideways,
                KeyCode::KeyD => horizontal_acceleration += sideways,
                KeyCode::Space => {
                    if player.is_flying {
                        player.velocity.y = JUMP_VELOCITY;
                    } else if player.is_swimming {
                        vertical_acceleration.y = 20.0
                    } else if player.is_grounded.y && last_jump.elapsed().as_secs_f32() > JUMP_TIME
                    {
                        last_jump.last = std::time::Instant::now();
                        player.velocity.y = JUMP_VELOCITY;
                    }
                }
                KeyCode::ShiftLeft => {
                    if player.is_flying {
                        player.velocity.y = -JUMP_VELOCITY;
                    } else if player.is_swimming {
                        vertical_acceleration.y = -30.0
                    }
                }

                _ => (),
            }
        }
    }

    if horizontal_acceleration != Vec3::ZERO {
        horizontal_acceleration = horizontal_acceleration.normalize();
    }

    let mut acceleration = horizontal_acceleration + vertical_acceleration;

    if player.is_flying && keys.pressed(KeyCode::ControlLeft) {
        acceleration *= 10.0;
    }

    if player.is_flying {
        acceleration *= 140.0;
    } else if player.is_swimming {
        if acceleration.y == 0.0 {
            acceleration.y = -10.0;
        }
        acceleration.x *= 40.0;
        acceleration.z *= 40.0;
    } else if player.is_grounded.y {
        acceleration *= 100.0;
    } else if player.velocity.x.abs() > 2.0
        || player.velocity.z.abs() > 2.0
        || player.velocity.y < -10.0
    {
        // Move fast in air if you're already in motion
        acceleration *= 50.0;
    } else {
        // Move slow in air in jumping from a standstill
        acceleration *= 20.0;
    }

    if !player.is_flying && !player.is_swimming {
        acceleration += GRAVITY;
    }

    player.acceleration = acceleration;
}

// TODO: If you travel more than 0.5 blocks per tick you will tunnel.
fn simulate_player_physics(
    origin: Res<Origin>,
    world_map: Res<WorldMap>,
    fixed_time: Res<Time>,
    net: Res<NetworkClient>,
    mut player: Query<(&mut PlayerState, &mut Transform, &Aabb)>,
    mut last_position_sent_to_server: Local<Vec3>,
) {
    let (mut player, mut transform, player_aabb) = player.single_mut();
    let delta_time = fixed_time.delta_seconds();

    if player.velocity.x != 0.0 {
        player.is_grounded.x = false;
    }
    if player.velocity.y != 0.0 {
        player.is_grounded.y = false;
    }
    if player.velocity.z != 0.0 {
        player.is_grounded.z = false;
    }

    let accel = player.acceleration;
    player.velocity += accel * delta_time;

    let mut friction = Vec3::ZERO;
    for velocity in [
        Vec3::new(0.0, player.velocity.y, 0.0),
        Vec3::new(player.velocity.x, 0.0, 0.0),
        Vec3::new(0.0, 0.0, player.velocity.z),
    ] {
        let pos_after_move = transform.translation + velocity * delta_time;

        let player_aabb = Aabb {
            center: player_aabb.center + Vec3A::from(pos_after_move),
            half_extents: player_aabb.half_extents,
        };

        // Check for collisions for all blocks within the player's aabb.
        let mut collisions = Vec::new();
        let start = player_aabb.min().floor().as_ivec3() + origin.0;
        let stop = player_aabb.max().floor().as_ivec3() + origin.0;
        for x in start.x..=stop.x {
            for y in start.y..=stop.y {
                for z in start.z..=stop.z {
                    let block_pos = IVec3::new(x, y, z);

                    let block_id = match world_map.get_block(&block_pos) {
                        Some(id) => id,
                        // Disconnect? Should always have your surroundings loaded.
                        None => continue,
                    };

                    let block_aabb = Aabb {
                        center: (block_pos - origin.0).as_vec3a() + 0.5,
                        half_extents: Vec3A::new(0.5, 0.5, 0.5),
                    };

                    let distance = player_aabb.center - block_aabb.center;
                    let overlap =
                        player_aabb.half_extents + block_aabb.half_extents - distance.abs();

                    if overlap.cmpgt(Vec3A::ZERO).all() {
                        // Keep sign to differentiate which side of the block was collided with.
                        collisions.push((Vec3::from(overlap.copysign(distance)), block_id));
                    }
                }
            }
        }

        let mut move_back = Vec3::ZERO;
        let delta_time = Vec3::splat(delta_time);

        let blocks = Blocks::get();

        // TODO: Some of this is leftover from converting from another system that didn't work as
        // as planned.
        for (collision, block_id) in collisions.clone() {
            let backwards_time = collision / -velocity;
            let valid_axes = backwards_time.cmplt(delta_time + delta_time / 100.0)
                & backwards_time.cmpgt(Vec3::splat(0.0));
            let resolution_axis = Vec3::select(valid_axes, backwards_time, Vec3::NAN).max_element();

            match blocks.get_config(block_id).friction() {
                Friction::Static {
                    front,
                    back,
                    right,
                    left,
                    top,
                    bottom,
                } => {
                    if resolution_axis == backwards_time.y {
                        move_back.y = collision.y + collision.y / 100.0;
                        player.is_grounded.y = true;
                        player.velocity.y = 0.0;

                        if velocity.y.is_sign_positive() {
                            friction = friction.max(Vec3::splat(*bottom));
                        } else {
                            friction = friction.max(Vec3::splat(*top));
                        }
                    } else if resolution_axis == backwards_time.x {
                        move_back.x = collision.x + collision.x / 100.0;
                        player.is_grounded.x = true;
                        player.velocity.x = 0.0;

                        if velocity.x.is_sign_positive() {
                            friction = friction.max(Vec3::splat(*left));
                        } else {
                            friction = friction.max(Vec3::splat(*right));
                        }
                    } else if resolution_axis == backwards_time.z {
                        move_back.z = collision.z + collision.z / 100.0;
                        player.is_grounded.z = true;
                        player.velocity.z = 0.0;

                        if velocity.z.is_sign_positive() {
                            friction = friction.max(Vec3::splat(*back));
                        } else {
                            friction = friction.max(Vec3::splat(*front));
                        }
                    } else {
                        // When velocity is really small there's numerical precision problems. Since a
                        // resolution is guaranteed. Move it back by whatever the smallest resolution
                        // direction is.
                        let valid_axes = Vec3::select(
                            backwards_time.cmpgt(Vec3::ZERO)
                                & backwards_time.cmplt(delta_time * 2.0),
                            backwards_time,
                            Vec3::NAN,
                        );
                        if valid_axes.x.is_finite()
                            || valid_axes.y.is_finite()
                            || valid_axes.z.is_finite()
                        {
                            let valid_axes = Vec3::select(
                                valid_axes.cmpeq(Vec3::splat(valid_axes.min_element())),
                                valid_axes,
                                Vec3::ZERO,
                            );
                            move_back += (valid_axes + valid_axes / 100.0) * -velocity;
                        }
                    }
                }
                Friction::Drag(drag) => {
                    friction = friction.max(*drag);
                }
            }
        }

        if transform.translation != pos_after_move + move_back {
            transform.translation = pos_after_move + move_back;
        }
    }

    // XXX: Pow(4) is just to scale it further towards zero when friction is high. The function
    // should be read as 'velocity *= friction^time'
    player.velocity = player.velocity * (1.0 - friction).powf(4.0).powf(delta_time);

    // Avoid sending constant position updates to the server.
    if (*last_position_sent_to_server - transform.translation)
        .abs()
        .cmpgt(Vec3::splat(0.01))
        .any()
    {
        *last_position_sent_to_server = transform.translation;
        net.send_message(messages::PlayerPosition {
            position: transform.translation.as_dvec3() + origin.as_dvec3(),
            velocity: player.velocity.as_dvec3(),
        });
    }
}

fn swimming(
    origin: Res<Origin>,
    world_map: Res<WorldMap>,
    mut player: Query<(&mut PlayerState, &Transform, &Aabb)>,
) {
    let (mut player, transform, player_aabb) = player.single_mut();

    let was_swimming = player.is_swimming;
    player.is_swimming = false;

    let player_aabb = Aabb {
        center: player_aabb.center + Vec3A::from(transform.translation),
        half_extents: player_aabb.half_extents,
    };

    let mut collisions = Vec::new();
    let start = player_aabb.min().floor().as_ivec3() + origin.0;
    let stop = player_aabb.max().floor().as_ivec3() + origin.0;
    for x in start.x..=stop.x {
        for y in start.y..=stop.y {
            for z in start.z..=stop.z {
                let block_pos = IVec3::new(x, y, z);

                let block_id = match world_map.get_block(&block_pos) {
                    Some(id) => id,
                    // Disconnect? Should always have your surroundings loaded.
                    None => continue,
                };

                let block_aabb = Aabb {
                    center: (block_pos - origin.0).as_vec3a() + 0.5,
                    half_extents: Vec3A::new(0.5, 0.5, 0.5),
                };

                let distance = player_aabb.center - block_aabb.center;
                let overlap = player_aabb.half_extents + block_aabb.half_extents - distance.abs();

                if overlap.cmpgt(Vec3A::ZERO).all() {
                    collisions.push(block_id);
                }
            }
        }
    }

    let blocks = Blocks::get();

    for block_id in collisions.clone() {
        match blocks.get_config(block_id).friction() {
            Friction::Static { .. } => (),
            Friction::Drag(drag) => {
                if !player.is_swimming {
                    player.is_swimming = drag.y > 0.4;
                }
            }
        }
    }

    // Give a little boost when exiting water so that the bob stays constant.
    if was_swimming && !player.is_swimming {
        player.velocity.y += 1.5;
    }
}
