// To make movement as smooth as possible, all player related physics are handled client side.
// Only the player's updated position is sent to the server.

//use std::sync::Arc;
//
//use bevy::{time::FixedTimestep, math::Vec3A, prelude::*, render::primitives::Aabb};
//use fmc_networking::messages::ServerConfig;
//
//use crate::{
//    game_state::GameState,
//    player::Player,
//    world::{
//        blocks::{Block, BlockSide, Blocks},
//        world_map::WorldMap,
//    },
//};

// Objects shouldn't move more than half a block(0.5) per game tick. At 1s/120 that is 60m/s
// The 0.5 limit is used to know which side of a block the object collided with.
//const TERMINAL_VELOCITY: f64 = 60.0;
//const TIMESTEP: f64 = 1.0 / 120.0;

//pub struct PhysicsPlugin;
//impl Plugin for PhysicsPlugin {
//    fn build(&self, app: &mut App) {
//        app.add_system_set(
//            // Same as below
//            //CoreStage::PostUpdate,
//            SystemSet::on_update(GameState::Playing)
//                // TODO: Waiting for bevy to implement run criteria composition
//                // https://github.com/bevyengine/rfcs/pull/45
//                //.with_run_criteria(FixedTimestep::step(TIMESTEP))
//                .with_system(simulate_player_physics_2),
//        );
//    }
//}

///// Collision detection and movement of player.
//// TODO: This needs to be redone. It was meant to be a simple system for the prototype but got
//// confusing as parts were added and removed to make it work. It's unknown which parts are needed
//// and which are not. The original idea was to move the player out of any collisions by going the
//// opposite way of the velocity vector on initial collision and then setting 'grounded' in the
//// directions it collided. Then on subsequent collisions on the grounded axis, move it along the
//// axis.
//// This can be visualized, if a player comes from above in an arch and collides with the ground it
//// is moved out by '-velocity_vector * xyz' which will put it at the origin of the collision. If you
//// then walk somwhere the velocity vector will be something like (5.0, -0.02, 0) the y value being
//// gravity. The next step the player will collide with the ground again, but will be moved back up
//// by '-(-0.02) * y' this time, as it is grounded. If it would have used the same formula as on initial
//// collision, it would impossible to move as you would also revert the x-direction movement.
//// TODO: At high velocity this will wormhole or embed the player in the terrain, use TERMINAL_VELOCITY
//// TODO: When this encounters an unsolveable collision where the player should be stuck, it will
//// fuck up. Should instead suffocate.
//fn simulate_player_physics(
//    world_map: Res<WorldMap>,
//    time: Res<Time>,
//    server_config: Res<ServerConfig>,
//    blocks: Res<Arc<Blocks>>,
//    mut player: Query<(&mut Player, &mut Transform, &Aabb)>,
//) {
//    let (mut player, mut transform, player_aabb) = player.single_mut();
//
//    // Finds the overlap between a block and an aabb, if there is one.
//    // Returns Some if there is an overlap, None if there isn't.
//    // TODO: This could need a check for the block actually existing, it is currently assumed
//    // loaded as the chunk_manager::proximity_chunk_loading loads all the chunks in proximity of
//    // the player. That is not robust though.
//    let test_block_collision =
//        |block_pos: IVec3, aabb: &Aabb, velocity: &Vec3| -> Option<(Vec3, [bool; 3], &BlockInfo)> {
//            let block_id = if let Some(block_id) = world_map.get_block(&block_pos) {
//                // TODO: Test only blocks that can be collided with
//                if block_id != 0 {
//                    block_id
//                } else {
//                    return None;
//                }
//            } else {
//                // TODO: Idk, this might also fail if the server is slow to respond with chunk updates.
//                // This should probably be a Result, just skip the system if error.
//                return None;
//                //panic!("Expected to find a block, as the chunks around the player should always be in memory");
//            };
//
//            let block_aabb = Aabb {
//                center: block_pos.as_vec3a() + 0.5,
//                half_extents: Vec3A::new(0.5, 0.5, 0.5),
//            };
//
//            let overlap: Vec3 = (aabb.half_extents + block_aabb.half_extents
//                - (aabb.center - block_aabb.center).abs())
//            .into();
//
//            if overlap.cmpgt(Vec3::ZERO).all() {
//                // Is false e.g if the player is moving upwards and collides with the top side of a block.
//                let which_direction_does_it_overlap = (block_aabb.center - aabb.center)
//                    .signum()
//                    .cmpeq(velocity.signum().into())
//                    .into();
//                return Some((
//                    overlap,
//                    which_direction_does_it_overlap,
//                    blocks.get(&block_id).unwrap(),
//                ));
//            } else {
//                return None;
//            }
//        };
//
//    // Move player and then check that it does not collide with anything.
//    //transform.translation += player.velocity * TIMESTEP as f32;
//    transform.translation += player.velocity * time.delta_seconds();
//
//    // The player position is never rotated, so this will just shift the position.
//    let mut player_aabb = Aabb::from_min_max(
//        transform.mul_vec3(player_aabb.min().into()),
//        transform.mul_vec3(player_aabb.max().into()),
//    );
//    let mut friction: f32 = 0.0;
//
//    // Check the for collisions in all blocks within the player's aabb.
//    let start = player_aabb.min().floor().as_ivec3();
//    let stop = player_aabb.max().floor().as_ivec3();
//
//    let prev_velocity = player.velocity;
//    let mut moved_x = false;
//    let mut moved_y = false;
//    let mut moved_z = false;
//    for x in start.x..=stop.x {
//        for y in start.y..=stop.y {
//            for z in start.z..=stop.z {
//                if let Some((overlap, overlap_dir, block_info)) =
//                    test_block_collision(IVec3::new(x, y, z), &player_aabb, &player.velocity)
//                {
//                    // This is how much we need to go back along the velocity vector to get
//                    // outside of the block again.
//                    let backtrack = (overlap / prev_velocity).abs();
//                    let which_axis = backtrack.min_element();
//
//                    if !which_axis.is_finite() {
//                        continue;
//                    }
//
//                    // TODO: Better way to do it without comparing numbers?
//                    if which_axis == backtrack.x && !moved_x && overlap_dir[0] {
//                        if player.grounded_x {
//                            transform.translation.x += -player.velocity.x * backtrack.x;
//                            player_aabb.center.x += -player.velocity.x * backtrack.x;
//                        } else {
//                            transform.translation += -player.velocity * backtrack.x;
//                            player_aabb.center += Vec3A::from(-player.velocity) * backtrack.x;
//                        }
//
//                        player.grounded_x = true;
//                        moved_x = true;
//                        player.velocity.x = 0.0;
//                    } else if which_axis == backtrack.y && !moved_y && overlap_dir[1] {
//                        if player.grounded_y {
//                            transform.translation.y += -player.velocity.y * backtrack.y;
//                            player_aabb.center.y += -player.velocity.y * backtrack.y;
//                        } else {
//                            transform.translation += -player.velocity * backtrack.y;
//                            player_aabb.center += Vec3A::from(-player.velocity) * backtrack.y;
//                        }
//
//                        player.grounded_y = true;
//                        moved_y = true;
//                        player.velocity.y = 0.0;
//                        friction = block_info.friction(BlockSide::Top);
//                        //dbg!(player.velocity, transform.translation);
//                    } else if which_axis == backtrack.z && !moved_z && overlap_dir[2] {
//                        if player.grounded_z {
//                            transform.translation.z += -player.velocity.z * backtrack.z;
//                            player_aabb.center.z += -player.velocity.z * backtrack.z;
//                        } else {
//                            transform.translation += -player.velocity * backtrack.z;
//                            player_aabb.center += Vec3A::from(-player.velocity) * backtrack.z;
//                        }
//
//                        player.grounded_z = true;
//                        moved_z = true;
//                        player.velocity.z = 0.0;
//                    }
//
//                    if !transform.translation.is_finite() {
//                        dbg!(
//                            transform.translation,
//                            player.velocity,
//                            overlap,
//                            overlap_dir,
//                            moved_x,
//                            moved_z,
//                            moved_y
//                        );
//                    }
//                }
//            }
//        }
//    }
//
//    player.velocity = player.velocity * (1.0 - friction);
//
//    if player.grounded_x && player.velocity.x != 0.0 {
//        player.grounded_x = false;
//    }
//    if player.grounded_y && player.velocity.y != 0.0 {
//        player.grounded_y = false;
//    }
//    if player.grounded_z && player.velocity.z != 0.0 {
//        player.grounded_z = false;
//    }
//
//    if !player.flying {
//        player.velocity += server_config.gravity * time.delta_seconds();
//    }
//}
