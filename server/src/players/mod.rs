use fmc::{
    bevy::math::{DQuat, DVec3},
    blocks::Blocks,
    database::Database,
    items::ItemStack,
    physics::shapes::Aabb,
    players::{Camera, Player},
    prelude::*,
    utils,
    world::{chunk::Chunk, WorldMap},
};
use fmc_networking::{messages, ConnectionId, NetworkServer, ServerNetworkEvent, Username};
use serde::{Deserialize, Serialize};

use crate::{items::crafting::CraftingGrid, world::WorldProperties};

use self::health::Health;

mod hand;
mod health;
mod inventory_interface;

pub use hand::HandInteractions;

pub struct PlayerPlugin;
impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<RespawnEvent>()
            .add_plugins(inventory_interface::InventoryInterfacePlugin)
            .add_plugins(health::HealthPlugin)
            .add_plugins(hand::HandPlugin)
            .add_systems(
                Update,
                (
                    remove_players,
                    (add_players, apply_deferred).chain(),
                    respawn_players,
                ),
            )
            // Save player after all remaining events have been handled. Avoid dupes and other
            // unexpected behaviour.
            .add_systems(PostUpdate, remove_players);
    }
}

#[derive(Component)]
enum GameMode {
    Survival,
    Creative,
}

#[derive(Component, Serialize, Deserialize, Deref, DerefMut, Clone)]
pub struct Inventory(Vec<ItemStack>);

impl Default for Inventory {
    fn default() -> Self {
        Self(vec![ItemStack::default(); 36])
    }
}

/// Helmet, chestplate, leggings, boots in order
#[derive(Component, Default, Serialize, Deserialize, Clone)]
pub struct Equipment {
    helmet: ItemStack,
    chestplate: ItemStack,
    leggings: ItemStack,
    boots: ItemStack,
}

#[derive(Component, Default, Serialize, Deserialize)]
pub struct EquippedItem(pub usize);

/// Default bundle used for new players.
#[derive(Bundle)]
pub struct PlayerBundle {
    transform: Transform,
    camera: Camera,
    aabb: Aabb,
    inventory: Inventory,
    equipment: Equipment,
    crafting_table: CraftingGrid,
    equipped_item: EquippedItem,
    health: Health,
    gamemode: GameMode,
}

impl Default for PlayerBundle {
    fn default() -> Self {
        Self {
            transform: Transform::default(),
            camera: Camera::default(),
            aabb: Aabb::from_min_max(DVec3::new(-0.3, 0.0, -0.3), DVec3::new(0.3, 1.8, 0.3)),
            inventory: Inventory::default(),
            equipment: Equipment::default(),
            crafting_table: CraftingGrid::with_size(4),
            equipped_item: EquippedItem::default(),
            health: Health {
                hearts: 20,
                max: 20,
            },
            gamemode: GameMode::Survival,
        }
    }
}

/// The format the player is saved as in the database.
#[derive(Serialize, Deserialize)]
pub struct PlayerSave {
    position: DVec3,
    camera_position: DVec3,
    camera_rotation: DQuat,
    inventory: Inventory,
    equipment: Equipment,
    health: Health,
}

impl PlayerSave {
    fn save(&self, username: &str, database: &Database) {
        let conn = database.get_connection();

        let mut stmt = conn
            .prepare("INSERT OR REPLACE INTO players VALUES (?,?)")
            .unwrap();
        stmt.execute(rusqlite::params![
            username,
            serde_json::to_string(self).unwrap()
        ])
        .unwrap();
    }

    fn load(username: &str, database: &Database) -> Option<Self> {
        let conn = database.get_connection();

        let mut stmt = conn
            .prepare("SELECT save FROM players WHERE name = ?")
            .unwrap();
        let mut rows = if let Ok(rows) = stmt.query([username]) {
            rows
        } else {
            return None;
        };

        // TODO: I've forgot how you're supposed to do this correctly
        if let Some(row) = rows.next().unwrap() {
            let json: String = row.get_unwrap(0);
            let save: PlayerSave = serde_json::from_str(&json).unwrap();
            return Some(save);
        } else {
            return None;
        };
    }

    // TODO: Remember equipped and send to player
    fn to_bundle(self) -> PlayerBundle {
        PlayerBundle {
            transform: Transform::from_translation(self.position),
            camera: Camera(Transform {
                translation: self.camera_position,
                rotation: self.camera_rotation,
                ..default()
            }),
            inventory: self.inventory,
            equipment: self.equipment,
            health: self.health,
            ..default()
        }
    }
}

fn add_players(
    mut commands: Commands,
    net: Res<NetworkServer>,
    database: Res<Database>,
    mut respawn_events: EventWriter<RespawnEvent>,
    added_players: Query<(Entity, &Username, &ConnectionId), Added<Player>>,
) {
    for (entity, username, connection) in added_players.iter() {
        let bundle = if let Some(save) = PlayerSave::load(&username, &database) {
            save.to_bundle()
        } else {
            respawn_events.send(RespawnEvent { entity });
            PlayerBundle::default()
        };

        net.send_one(
            *connection,
            messages::PlayerPosition {
                position: bundle.transform.translation,
                velocity: DVec3::ZERO,
            },
        );

        net.send_one(
            *connection,
            messages::PlayerCameraPosition {
                position: bundle.camera.translation.as_vec3(),
            },
        );

        net.send_one(
            *connection,
            messages::PlayerCameraRotation {
                rotation: bundle.camera.rotation.as_quat(),
            },
        );

        commands.entity(entity).insert(bundle);
    }
}

fn remove_players(
    database: Res<Database>,
    mut network_events: EventReader<ServerNetworkEvent>,
    players: Query<(
        &Username,
        &Transform,
        &Camera,
        &Inventory,
        &Equipment,
        &Health,
    )>,
) {
    for network_event in network_events.read() {
        let ServerNetworkEvent::Disconnected { entity } = network_event else {
            continue;
        };

        let Ok((username, transform, camera, inventory, equipment, health)) = players.get(*entity)
        else {
            continue;
        };

        PlayerSave {
            position: transform.translation,
            camera_position: camera.translation,
            camera_rotation: camera.rotation,
            inventory: inventory.clone(),
            equipment: equipment.clone(),
            health: health.clone(),
        }
        .save(&username, &database);
    }
}

#[derive(Event)]
pub struct RespawnEvent {
    pub entity: Entity,
}

// TODO: If it can't find a valid spawn point it will just oscillate in an infinite loop between the
// air chunk above and the one it can't find anything in.
// TODO: This might take a really long time to compute because of the chunk loading, and should
// probably be done ahead of time through an async task. Idk if the spawn point should change
// between each spawn. A good idea if it's really hard to validate that the player won't suffocate
// infinitely.
fn respawn_players(
    net: Res<NetworkServer>,
    world_properties: Res<WorldProperties>,
    world_map: Res<WorldMap>,
    database: Res<Database>,
    mut respawn_events: EventReader<RespawnEvent>,
    connection_query: Query<&ConnectionId>,
) {
    for event in respawn_events.read() {
        let blocks = Blocks::get();
        let air = blocks.get_id("air");

        let mut chunk_position =
            utils::world_position_to_chunk_position(world_properties.spawn_point.center);
        let spawn_position = 'outer: loop {
            let chunk = futures_lite::future::block_on(Chunk::load(
                chunk_position,
                world_map.terrain_generator.clone(),
                database.clone(),
            ))
            .1;

            if chunk.is_uniform() && chunk[0] == air {
                break chunk_position;
            }

            // Find a spot that has a block with two air blocks above.
            for (i, block_chunk) in chunk.blocks.chunks_exact(Chunk::SIZE).enumerate() {
                let mut count = 0;
                for (j, block) in block_chunk.iter().enumerate() {
                    //if count == 3 {
                    if count == 2 {
                        let mut spawn_position =
                            chunk_position + utils::block_index_to_position(i * Chunk::SIZE + j);
                        spawn_position.y -= 2;
                        break 'outer spawn_position;
                    //} else if count == 0 && *block != air {
                    } else if count == 3 && *block != air {
                        count += 1;
                        //match blocks.get_config(&block).friction {
                        //    Friction::Drag(_) => continue,
                        //    _ => count += 1,
                        //};
                        //} else if count == 1 && *block == air {
                    } else if count == 0 && *block == air {
                        count += 1;
                    //} else if count == 2 && *block == air {
                    } else if count == 1 && *block == air {
                        count += 1;
                    } else {
                        count = 0;
                    }
                }
            }

            chunk_position.y += Chunk::SIZE as i32;
        };

        let connection_id = connection_query.get(event.entity).unwrap();
        net.send_one(
            *connection_id,
            messages::PlayerPosition {
                position: spawn_position.as_dvec3(),
                velocity: DVec3::ZERO,
            },
        );
    }
}
