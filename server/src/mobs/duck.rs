use std::collections::HashSet;

use fmc::{
    bevy::math::DVec3,
    blocks::Blocks,
    items::Items,
    models::{Model, ModelAnimations, ModelBundle, ModelVisibility, Models},
    physics::{Acceleration, Buoyancy, PhysicsBundle, Velocity},
    players::Player,
    prelude::*,
    world::WorldMap,
};
use rand::Rng;

use crate::players::{EquippedItem, HandInteractions, Inventory};

use super::pathfinding::PathFinder;

pub struct DuckPlugin;
impl Plugin for DuckPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                spawn_duck,
                remove_duck,
                wander,
                move_to_pathfinding_goal,
                beg_for_bread,
                handle_interactions,
            ),
        );
    }
}

#[derive(Component, Default)]
struct Duck {
    _focus: Option<DVec3>,
    wander_timer: Timer,
    is_begging_from_player: bool,
}

fn spawn_duck(
    mut commands: Commands,
    world_map: Res<WorldMap>,
    models: Res<Models>,
    time: Res<Time>,
    duck: Query<Entity, With<Duck>>,
) {
    if time.elapsed_seconds() < 1.0 || duck.iter().count() == 1 {
        return;
    }
    if !world_map.contains_chunk(&IVec3::new(-16, 0, 0)) {
        return;
    }
    let duck_model = models.get_by_name("duck");

    let mut animations = ModelAnimations::default();
    animations.play_on_move(Some(duck_model.animations["walk"]));

    commands.spawn((
        Duck::default(),
        PhysicsBundle {
            aabb: duck_model.aabb.clone(),
            ..default()
        },
        Buoyancy {
            density: 0.3,
            waterline: 0.4,
        },
        ModelBundle {
            model: Model { id: duck_model.id },
            animations,
            visibility: ModelVisibility::default(),
            transform: Transform::from_xyz(-30.0, 2.0, -1.0),
            global_transform: GlobalTransform::default(),
        },
        PathFinder::new(1, 1),
        HandInteractions::default(),
    ));
}

fn remove_duck(
    mut commands: Commands,
    duck: Query<Entity, With<Duck>>,
    mut player: RemovedComponents<Player>,
) {
    for _removed in player.read() {
        commands.entity(duck.single()).despawn_recursive();
    }
}

fn beg_for_bread(
    world_map: Res<WorldMap>,
    items: Res<Items>,
    players: Query<(&Inventory, &EquippedItem, &GlobalTransform), With<Player>>,
    mut ducks: Query<(&mut Duck, &mut PathFinder, &GlobalTransform)>,
) {
    'outer: for (mut duck, mut path_finder, duck_transform) in ducks.iter_mut() {
        for (inventory, equipped_item, player_transform) in players.iter() {
            if duck_transform
                .translation()
                .distance_squared(player_transform.translation())
                > 25.0
            {
                continue;
            }

            let equipped_item = &inventory[equipped_item.0];
            if equipped_item.is_empty() {
                continue;
            }
            let item_id = equipped_item.item().unwrap().id;

            if items.get_id("bread").unwrap() != item_id {
                continue;
            }

            let mut offset = player_transform.translation() - duck_transform.translation();
            offset.y = 0.0;
            offset = offset.normalize();

            path_finder.find_path(
                &world_map,
                duck_transform.translation(),
                player_transform.translation() - offset,
            );

            duck.is_begging_from_player = true;
            continue 'outer;
        }

        duck.is_begging_from_player = false;
    }
}

fn wander(
    world_map: Res<WorldMap>,
    time: Res<Time>,
    mut ducks: Query<(&mut Duck, &mut PathFinder, &GlobalTransform)>,
) {
    for (mut duck, mut path_finder, transform) in ducks.iter_mut() {
        duck.wander_timer.tick(time.delta());

        if duck.is_begging_from_player {
            continue;
        }

        if duck.wander_timer.finished() {
            duck.wander_timer =
                Timer::from_seconds(rand::thread_rng().gen_range(10.0..=15.0), TimerMode::Once);
        } else {
            continue;
        }

        let mut already_visited = HashSet::new();
        let mut potential_blocks = Vec::new();

        let blocks = Blocks::get();
        let water_id = blocks.get_id("surface_water");

        let start = transform.translation().floor().as_ivec3();
        // elements of (position, score)
        potential_blocks.push((start, u32::MIN, 0));
        already_visited.insert(start);

        let max_distance = rand::thread_rng().gen_range(1..=8);

        let mut index = 0;
        while let Some((block_position, mut score, mut distance)) =
            potential_blocks.get(index).cloned()
        {
            index += 1;

            distance += 1;
            if distance > max_distance {
                continue;
            }

            for offset in [IVec3::X, IVec3::NEG_X, IVec3::Z, IVec3::NEG_Z] {
                let block_position = block_position + offset;

                if !already_visited.insert(block_position) {
                    continue;
                }

                // Always increase score, to always move as far as possible
                score += 1;

                let Some(block_id) = world_map.get_block(block_position) else {
                    continue;
                };
                let block_config = blocks.get_config(&block_id);

                if block_config.is_solid() {
                    // Try to jump one block up
                    let above = block_position + IVec3::Y;
                    let block_config = if let Some(block_id) = world_map.get_block(above) {
                        blocks.get_config(&block_id)
                    } else {
                        continue;
                    };
                    if !block_config.is_solid() {
                        potential_blocks.push((above, score, distance));
                    }
                } else if block_id == water_id {
                    // If in water, stay in the shallows
                    for step in 1..3i32 {
                        let below = block_position - IVec3::Y * step;
                        let block_config = if let Some(block_id) = world_map.get_block(below) {
                            blocks.get_config(&block_id)
                        } else {
                            break;
                        };
                        if block_config.is_solid() {
                            potential_blocks.push((block_position, score, distance));
                            break;
                        }
                    }
                    potential_blocks.push((block_position, score, distance));
                } else {
                    for step in 1..=2i32 {
                        let below = block_position - IVec3::Y * step;
                        let block_config = if let Some(block_id) = world_map.get_block(below) {
                            blocks.get_config(&block_id)
                        } else {
                            break;
                        };

                        if block_config.is_solid() {
                            potential_blocks.push((below + IVec3::Y, score, distance));
                            break;
                        } else {
                            // Prefer walking down, will hopefully lead to the shore (or a hole if
                            // unlucky)
                            score += 1;
                        }
                    }
                }
            }
        }

        let mut best_position = None;
        let mut max_score = 0;
        for (block_position, score, _distance) in potential_blocks {
            if score > max_score {
                best_position = Some(block_position);
                max_score = score;
            }
        }

        if let Some(best_position) = best_position {
            let goal = best_position.as_dvec3() + DVec3::new(0.5, 0.0, 0.5);
            path_finder.find_path(&world_map, transform.translation(), goal);
        }
    }
}

// Formula for how much speed you need to reach a height
// sqrt(2 * gravity * wanted height(1.4)) + some for air resistance
const JUMP_VELOCITY: f64 = 9.0;
const WALK_ACCELERATION: f64 = 30.0;

fn move_to_pathfinding_goal(
    mut ducks: Query<
        (
            &mut PathFinder,
            &mut Acceleration,
            &mut Velocity,
            &mut Transform,
        ),
        (
            With<Duck>,
            Or<(Changed<GlobalTransform>, Changed<PathFinder>)>,
        ),
    >,
) {
    for (mut path_finder, mut acceleration, mut velocity, mut transform) in ducks.iter_mut() {
        if let Some(next_position) = path_finder.next_node(transform.translation) {
            // Only rotate around the Y-axis
            transform.look_at(next_position, DVec3::Y);
            transform.rotation.x = 0.0;
            transform.rotation.z = 0.0;
            transform.rotation = transform.rotation.normalize();

            let direction = (next_position - transform.translation).normalize();

            // TODO: Should not jump out of water, accelerate only so it looks more like a step up.
            if direction.y > 0.1 {
                if velocity.y < 0.1 {
                    velocity.y += JUMP_VELOCITY;
                }
                acceleration.x += direction.x * WALK_ACCELERATION;
                acceleration.z += direction.z * WALK_ACCELERATION;
            } else if acceleration.y.abs() < 0.2 {
                // TODO: This needs to be more deliberate. Needs states for when
                // grounded/swimming/falling and differing speeds.
                acceleration.x += direction.x * WALK_ACCELERATION;
                acceleration.z += direction.z * WALK_ACCELERATION;
            }
        }
    }
}

fn handle_interactions(
    items: Res<Items>,
    mut player_query: Query<(&mut Inventory, &EquippedItem), With<Player>>,
    mut ducks: Query<&mut HandInteractions, (With<Duck>, Changed<HandInteractions>)>,
) {
    for mut interactions in ducks.iter_mut() {
        for player_entity in interactions.read() {
            let (mut inventory, equipped_item) = player_query.get_mut(player_entity).unwrap();
            let item_stack = &mut inventory[equipped_item.0];

            if item_stack.is_empty() {
                continue;
            }

            if item_stack.item().unwrap().id != items.get_id("bread").unwrap() {
                continue;
            }

            item_stack.subtract(1);
        }
    }
}
