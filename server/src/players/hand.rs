use std::collections::HashMap;

use fmc_networking::{messages, NetworkData};

use fmc::{
    bevy::math::DVec3,
    blocks::{BlockFace, BlockId, BlockRotation, BlockState, Blocks, Friction},
    items::Items,
    models::{Model, ModelAnimations, ModelBundle, ModelMap, ModelVisibility, Models},
    physics::shapes::Aabb,
    players::{Camera, Player},
    prelude::*,
    utils,
    world::{chunk::Chunk, BlockUpdate, WorldMap},
};

use crate::{
    items::{GroundItemBundle, ItemUses, RegisterItemUse, UsableItems},
    players::{EquippedItem, Inventory},
};

pub struct HandPlugin;
impl Plugin for HandPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<BlockBreakingEvent>().add_systems(
            Update,
            (
                handle_left_clicks,
                (handle_right_clicks).in_set(RegisterItemUse),
                break_blocks.after(handle_left_clicks),
            ),
        );
    }
}

/// Together with an Aabb this tracks when a player right clicks an entity
#[derive(Component, Default)]
pub struct HandInteractions {
    player_entities: Vec<Entity>,
}

impl HandInteractions {
    pub fn read(&mut self) -> impl Iterator<Item = Entity> + '_ {
        self.player_entities.drain(..)
    }

    pub fn push(&mut self, player_entity: Entity) {
        self.player_entities.push(player_entity);
    }
}

#[derive(Event, Hash, Eq, PartialEq)]
struct BlockBreakingEvent {
    player_entity: Entity,
    block_position: IVec3,
    block_id: BlockId,
}

// Keeps the state of how far along a block is to breaking
#[derive(Debug)]
struct BreakingBlock {
    model_entity: Entity,
    progress: f32,
    prev_hit: std::time::Instant,
}

#[derive(Component)]
struct BreakingBlockMarker;

// TODO: Take into account player's equipped item
fn break_blocks(
    mut commands: Commands,
    items: Res<Items>,
    models: Res<Models>,
    player_equipped_item_query: Query<(&Inventory, &EquippedItem), With<Player>>,
    mut model_query: Query<(&mut Model, &mut ModelVisibility), With<BreakingBlockMarker>>,
    mut block_update_writer: EventWriter<BlockUpdate>,
    mut block_breaking_events: EventReader<BlockBreakingEvent>,
    mut being_broken: Local<HashMap<IVec3, BreakingBlock>>,
) {
    let now = std::time::Instant::now();

    let blocks = Blocks::get();

    for breaking_event in block_breaking_events.read() {
        // Guard against duplicate events, many left clicks often arrive at once.
        if let Some(breaking_block) = being_broken.get(&breaking_event.block_position) {
            if now == breaking_block.prev_hit {
                continue;
            }
        }

        let (inventory, equipped_item_index) = player_equipped_item_query
            .get(breaking_event.player_entity)
            .unwrap();
        let equipped_item_stack = &inventory[equipped_item_index.0];
        let tool = if let Some(item) = equipped_item_stack.item() {
            let equipped_item_config = items.get_config(&item.id);
            equipped_item_config.tool.as_ref()
        } else {
            None
        };

        let block_config = blocks.get_config(&breaking_event.block_id);

        if let Some(breaking_block) = being_broken.get_mut(&breaking_event.block_position) {
            if now == breaking_block.prev_hit {
                // Block has already been hit this tick
                continue;
            } else if (now - breaking_block.prev_hit).as_secs_f32() > 0.05 {
                // The interval between two clicks needs to be short in order to be counted as
                // holding the button down.
                breaking_block.prev_hit = now;
                continue;
            }

            let (mut model, mut visibility) =
                model_query.get_mut(breaking_block.model_entity).unwrap();

            let prev_progress = breaking_block.progress;

            // Hardness is 'time to break'. We know it's Some because only blocks with hardness can
            // be broken.
            breaking_block.progress += (now - breaking_block.prev_hit).as_secs_f32()
                / block_config.hardness.unwrap()
                * tool.map(|t| t.efficiency).unwrap_or(1.0);
            breaking_block.prev_hit = now;

            let progress = breaking_block.progress;

            // Ordering from high to low lets it skip stages.
            if progress >= 1.0 {
                block_update_writer.send(BlockUpdate::Change {
                    position: breaking_event.block_position,
                    block_id: blocks.get_id("air"),
                    block_state: None,
                });

                let block_config = blocks.get_config(&breaking_event.block_id);
                let (dropped_item_id, count) =
                    match block_config.drop(tool.map(|t| t.name.as_str())) {
                        Some(drop) => drop,
                        None => continue,
                    };
                let item_config = items.get_config(&dropped_item_id);
                let model_config = models.get_by_id(item_config.model_id);

                commands.spawn(GroundItemBundle::new(
                    dropped_item_id,
                    item_config,
                    model_config,
                    count,
                    breaking_event.block_position.as_dvec3(),
                ));
            } else if prev_progress < 0.9 && progress > 0.9 {
                model.id = models.get_by_name("breaking_stage_9").id;
            } else if prev_progress < 0.8 && progress > 0.8 {
                model.id = models.get_by_name("breaking_stage_8").id;
            } else if prev_progress < 0.7 && progress > 0.7 {
                model.id = models.get_by_name("breaking_stage_7").id;
            } else if prev_progress < 0.6 && progress > 0.6 {
                model.id = models.get_by_name("breaking_stage_6").id;
            } else if prev_progress < 0.5 && progress > 0.5 {
                model.id = models.get_by_name("breaking_stage_5").id;
            } else if prev_progress < 0.4 && progress > 0.4 {
                model.id = models.get_by_name("breaking_stage_4").id;
            } else if prev_progress < 0.3 && progress > 0.3 {
                model.id = models.get_by_name("breaking_stage_3").id;
            } else if prev_progress < 0.2 && progress > 0.2 {
                model.id = models.get_by_name("breaking_stage_2").id;
            } else if prev_progress < 0.1 && progress > 0.1 {
                visibility.is_visible = true;
            }
        } else if block_config.hardness.unwrap() == 0.0 {
            // Blocks that break instantly
            block_update_writer.send(BlockUpdate::Change {
                position: breaking_event.block_position,
                block_id: blocks.get_id("air"),
                block_state: None,
            });

            let block_config = blocks.get_config(&breaking_event.block_id);
            let (dropped_item_id, count) = match block_config.drop(tool.map(|t| t.name.as_str())) {
                Some(drop) => drop,
                None => continue,
            };
            let item_config = items.get_config(&dropped_item_id);
            let model_config = models.get_by_id(item_config.model_id);

            commands.spawn(GroundItemBundle::new(
                dropped_item_id,
                item_config,
                model_config,
                count,
                breaking_event.block_position.as_dvec3(),
            ));

            // Guard against the block being broken again on the same tick
            being_broken.insert(
                breaking_event.block_position,
                BreakingBlock {
                    model_entity: commands.spawn_empty().id(),
                    progress: 1.0,
                    prev_hit: now,
                },
            );
        } else {
            let model_entity = commands
                .spawn(ModelBundle {
                    model: Model {
                        id: models.get_by_name("breaking_stage_1").id,
                    },
                    animations: ModelAnimations::default(),
                    // The model shouldn't show until some progress has been made
                    visibility: ModelVisibility { is_visible: false },
                    global_transform: GlobalTransform::default(),
                    transform: Transform::from_translation(
                        breaking_event.block_position.as_dvec3() + DVec3::splat(0.5),
                    ),
                })
                .insert(BreakingBlockMarker)
                .id();

            being_broken.insert(
                breaking_event.block_position,
                BreakingBlock {
                    model_entity,
                    progress: 0.0,
                    prev_hit: now,
                },
            );
        }
    }

    // Remove break progress after not being hit for 0.5 seconds.
    being_broken.retain(|_, breaking_block| {
        let remove_timout = (now - breaking_block.prev_hit).as_secs_f32() > 0.5;
        let remove_broken = breaking_block.progress >= 1.0;

        if remove_timout || remove_broken {
            commands.entity(breaking_block.model_entity).despawn();
            return false;
        } else {
            return true;
        }
    });
}

// Left clicks are used for block breaking or attacking.
// TODO: Need spatial partitioning of item/mobs/players to do hit detection.
fn handle_left_clicks(
    mut clicks: EventReader<NetworkData<messages::LeftClick>>,
    world_map: Res<WorldMap>,
    player_query: Query<(&GlobalTransform, &Camera)>,
    mut block_breaking_events: EventWriter<BlockBreakingEvent>,
) {
    for click in clicks.read() {
        let (player_position, player_camera) = player_query.get(click.source.entity()).unwrap();

        let camera_transform = Transform {
            translation: player_position.translation() + player_camera.translation,
            rotation: player_camera.rotation,
            ..default()
        };

        let (block_position, block_id, _block_face, _distance) =
            match world_map.raycast_to_block(&camera_transform, 5.0) {
                Some(b) => b,
                None => continue,
            };

        // Handling many breaking events
        block_breaking_events.send(BlockBreakingEvent {
            player_entity: click.source.entity(),
            block_position,
            block_id,
        });
    }
}

fn handle_right_clicks(
    world_map: Res<WorldMap>,
    items: Res<Items>,
    model_map: Res<ModelMap>,
    usable_items: Res<UsableItems>,
    model_query: Query<(&Aabb, &GlobalTransform), With<Model>>,
    mut player_query: Query<
        (&mut Inventory, &EquippedItem, &GlobalTransform, &Camera),
        With<Player>,
    >,
    mut item_use_query: Query<&mut ItemUses>,
    mut hand_interaction_query: Query<&mut HandInteractions>,
    mut block_update_writer: EventWriter<BlockUpdate>,
    mut clicks: EventReader<NetworkData<messages::RightClick>>,
) {
    for right_click in clicks.read() {
        let (mut inventory, equipped_item, player_position, player_camera) =
            player_query.get_mut(right_click.source.entity()).unwrap();

        let camera_transform = Transform {
            translation: player_position.translation() + player_camera.translation,
            rotation: player_camera.rotation,
            ..default()
        };

        let block_hit = world_map.raycast_to_block(&camera_transform, 5.0);

        let block_hit_distance = if let Some((_, _, _, distance)) = block_hit {
            distance
        } else {
            f64::MAX
        };

        let mut model_hit = None;
        let chunk_position = utils::world_position_to_chunk_position(
            player_position.translation().floor().as_ivec3(),
        );
        for x_offset in [IVec3::X, IVec3::NEG_X, IVec3::ZERO] {
            for y_offset in [IVec3::Y, IVec3::NEG_Y, IVec3::ZERO] {
                for z_offset in [IVec3::Z, IVec3::NEG_Z, IVec3::ZERO] {
                    let chunk_position = chunk_position
                        + x_offset * Chunk::SIZE as i32
                        + y_offset * Chunk::SIZE as i32
                        + z_offset * Chunk::SIZE as i32;
                    let Some(model_entities) = model_map.get_entities(&chunk_position) else {
                        continue;
                    };
                    for model_entity in model_entities {
                        let Ok((aabb, model_transform)) = model_query.get(*model_entity) else {
                            continue;
                        };

                        let aabb = Aabb {
                            center: aabb.center + model_transform.translation(),
                            half_extents: aabb.half_extents,
                        };

                        let Some(distance) = aabb.ray_intersection(
                            camera_transform.translation,
                            camera_transform.forward(),
                        ) else {
                            continue;
                        };

                        if block_hit_distance < distance {
                            continue;
                        }

                        if let Some((_, closest_distance)) = model_hit {
                            if distance < closest_distance {
                                model_hit = Some((*model_entity, distance));
                            }
                        } else {
                            model_hit = Some((*model_entity, distance));
                        }
                    }
                }
            }
        }

        if let Some((model_entity, _distance)) = model_hit {
            if let Ok(mut hand_interaction) = hand_interaction_query.get_mut(model_entity) {
                hand_interaction.push(right_click.source.entity());
            }
            continue;
        }

        let Some((block_pos, _, block_face, _)) = block_hit else {
            continue;
        };

        // TODO: Needs an override, sneak = always place block
        // If the block can be interacted with, the click always counts as an interaction
        let (chunk_position, block_index) =
            utils::world_position_to_chunk_position_and_block_index(block_pos);
        let chunk = world_map.get_chunk(&chunk_position).unwrap();
        if let Some(block_entity) = chunk.block_entities.get(&block_index) {
            if let Ok(mut interactions) = hand_interaction_query.get_mut(*block_entity) {
                interactions.push(right_click.source.entity());
                continue;
            }
        }

        let equipped_item = &mut inventory[equipped_item.0];

        if equipped_item.is_empty() {
            continue;
        }

        let item_id = equipped_item.item().unwrap().id;

        if let Some(item_use_entity) = usable_items.get(&item_id) {
            let mut uses = item_use_query.get_mut(*item_use_entity).unwrap();
            uses.push(
                right_click.source.entity(),
                block_hit.map(|(block_position, block_id, _, _)| (block_id, block_position)),
            );
        }

        let blocks = Blocks::get();

        let new_block_position = block_face.shift_position(block_pos);
        let replaced_block_id = world_map.get_block(new_block_position).unwrap();

        let replaced_block_config = blocks.get_config(&replaced_block_id);

        // Make sure the block we're replacing is not solid
        if !matches!(replaced_block_config.friction, Friction::Drag(_)) {
            continue;
        }

        let item_config = items.get_config(&item_id);

        let Some(block_id) = item_config.block else {
            continue;
        };

        equipped_item.subtract(1);

        // TODO: Placing blocks like stairs can be annoying, as situations often arise where your
        // position alone isn't adequate to find the correct placement.
        // There's a clever way to do this I think. If you partition a block face like this:
        //  -------------------
        //  | \_____________/ |
        //  | |             | |
        //  | |             | |
        //  | |             | |
        //  | |             | |
        //  | |_____________| |
        //  |/              \ |
        //  -------------------
        //  (4 outer trapezoids and one inner square)
        // By comparing which sector was clicked and the angle of the camera I think a more
        // intuitive block placement can be achieved.
        let block_state = if blocks.get_config(&block_id).is_rotatable {
            let mut block_state = BlockState::default();

            if block_face == BlockFace::Bottom {
                let distance = player_position.translation().as_ivec3() - block_pos;
                let max = IVec2::new(distance.x, distance.z).max_element();

                if max == distance.x {
                    if distance.x.is_positive() {
                        block_state.set_rotation(BlockRotation::Once);
                        Some(block_state)
                    } else {
                        block_state.set_rotation(BlockRotation::Thrice);
                        Some(block_state)
                    }
                } else if max == distance.z {
                    if distance.z.is_positive() {
                        None
                    } else {
                        block_state.set_rotation(BlockRotation::Twice);
                        Some(block_state)
                    }
                } else {
                    unreachable!()
                }
            } else {
                block_state.set_rotation(block_face.to_rotation());
                Some(block_state)
            }
        } else {
            None
        };

        block_update_writer.send(BlockUpdate::Change {
            position: new_block_position,
            block_id,
            block_state,
        });
    }
}
