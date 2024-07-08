use fmc::{
    bevy::math::DVec3,
    items::{Item, ItemConfig, ItemId, ItemStack, Items},
    models::{Model, ModelAnimations, ModelBundle, ModelConfig, ModelMap, ModelVisibility},
    physics::{PhysicsBundle, Velocity},
    prelude::*,
    utils,
};

use crate::players::Inventory;

pub struct GroundItemPlugin;
impl Plugin for GroundItemPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, pick_up_items);
    }
}

#[derive(Bundle)]
pub struct GroundItemBundle {
    dropped_item: DroppedItem,
    model_bundle: ModelBundle,
    physics_bundle: PhysicsBundle,
}

impl GroundItemBundle {
    pub fn new(
        item_id: ItemId,
        item_config: &ItemConfig,
        model_config: &ModelConfig,
        count: u32,
        position: DVec3,
    ) -> Self {
        let dropped_item = DroppedItem(ItemStack::new(
            Item::new(item_id),
            count,
            item_config.max_stack_size,
        ));

        let mut aabb = model_config.aabb.clone();

        // We want to scale the model down to fit in a 0.15xYx0.15 box so the dropped
        // item is fittingly small. Then extending the smallest horizontal dimension so
        // that it becomes square.
        const WIDTH: f64 = 0.075;
        let max = aabb.half_extents.x.max(aabb.half_extents.z);
        let scale = WIDTH / max;
        aabb.half_extents.x = WIDTH;
        aabb.half_extents.y *= scale;
        aabb.half_extents.z = WIDTH;

        let random = rand::random::<f64>() * std::f64::consts::TAU;
        let (velocity_x, velocity_z) = random.sin_cos();

        // XXX: For some reason the center has to be zeroed. Does bevy center gltf models?
        // When the model is scaled does it shift the center(zeroing it like this would
        // then be slightly off)?
        aabb.center *= 0.0;
        let translation = position + DVec3::splat(0.5) - DVec3::from(aabb.center);
        //Offset the aabb slightly downwards to make the item float for clients.
        aabb.center += DVec3::new(0.0, -0.1, 0.0);

        let model_bundle = ModelBundle {
            model: Model {
                id: item_config.model_id,
            },
            animations: ModelAnimations::default(),
            visibility: ModelVisibility { is_visible: true },
            global_transform: GlobalTransform::default(),
            transform: Transform {
                translation,
                scale: DVec3::splat(scale),
                ..default()
            },
        };

        let physics_bundle = PhysicsBundle {
            velocity: Velocity(DVec3::new(velocity_x, 5.5, velocity_z)),
            aabb,
            ..default()
        };

        return GroundItemBundle {
            dropped_item,
            model_bundle,
            physics_bundle,
        };
    }
}

// An item that is dropped on the ground.
#[derive(Component, Deref, DerefMut)]
struct DroppedItem(pub ItemStack);

fn pick_up_items(
    mut commands: Commands,
    model_map: Res<ModelMap>,
    items: Res<Items>,
    mut players: Query<(&GlobalTransform, &mut Inventory), Changed<GlobalTransform>>,
    mut dropped_items: Query<(Entity, &mut DroppedItem, &Transform)>,
) {
    for (player_position, mut player_inventory) in players.iter_mut() {
        let chunk_position =
            utils::world_position_to_chunk_position(player_position.translation().as_ivec3());
        let item_entities = match model_map.get_entities(&chunk_position) {
            Some(e) => e,
            None => continue,
        };

        'outer: for item_entity in item_entities.iter() {
            if let Ok((entity, mut dropped_item, transform)) = dropped_items.get_mut(*item_entity) {
                if transform
                    .translation
                    .distance_squared(player_position.translation())
                    < 2.0
                {
                    let item_config = items.get_config(&dropped_item.item().unwrap().id);

                    // First test that the item can be picked up. This is to avoid triggering
                    // change detection for the inventory. If detection is triggered, it will send
                    // an interface update to the client. Can't pick up = spam
                    let mut capacity = 0;
                    for item_stack in player_inventory.iter() {
                        if item_stack.item() == dropped_item.item() {
                            capacity += item_stack.capacity();
                        } else if item_stack.is_empty() {
                            capacity += item_config.max_stack_size;
                        }
                    }
                    if capacity == 0 {
                        break;
                    }

                    for item_stack in player_inventory.iter_mut() {
                        if let Some(item) = item_stack.item() {
                            if item != dropped_item.item().unwrap() || item_stack.capacity() == 0 {
                                continue;
                            }
                            dropped_item.transfer(item_stack, u32::MAX);
                        }

                        if dropped_item.is_empty() {
                            commands.entity(entity).despawn();
                            continue 'outer;
                        }
                    }

                    // Iterate twice to first fill up existing stacks before filling empty ones.
                    for item_stack in player_inventory.iter_mut() {
                        if item_stack.is_empty() {
                            *item_stack = ItemStack::new(
                                dropped_item.item().unwrap().clone(),
                                0,
                                item_config.max_stack_size,
                            );
                            dropped_item.transfer(item_stack, u32::MAX);
                        }

                        if dropped_item.is_empty() {
                            commands.entity(entity).despawn();
                            continue 'outer;
                        }
                    }
                }
            }
        }
    }
}
