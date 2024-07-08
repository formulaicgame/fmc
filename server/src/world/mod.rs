use fmc::{blocks::Blocks, database::Database, prelude::*, world::WorldMap};
use serde::{Deserialize, Serialize};

use crate::settings::Settings;

mod biomes;
pub mod blocks;
mod terrain_generation;

pub struct WorldPlugin;
impl Plugin for WorldPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(blocks::BlocksPlugin)
            .add_systems(Startup, setup)
            .add_systems(
                Update,
                save_world_properties.run_if(resource_changed::<WorldProperties>),
            );
    }
}

fn setup(
    mut commands: Commands,
    database: Res<Database>,
    blocks: Res<Blocks>,
    settings: Res<Settings>,
) {
    let properties = WorldProperties::load(database).unwrap_or(WorldProperties::default());
    commands.insert_resource(properties);

    commands.insert_resource(WorldMap::new(
        terrain_generation::Earth::new(0, &blocks),
        settings.render_distance,
    ));
}

fn save_world_properties(database: Res<Database>, properties: Res<WorldProperties>) {
    properties.save(database);
}

#[derive(Default, Serialize, Deserialize, Resource)]
pub struct WorldProperties {
    // TODO: This must be set to a valid spawn point when first inserted, currently it is just
    // ignored.
    pub spawn_point: SpawnPoint,
}

impl WorldProperties {
    fn load(database: Res<Database>) -> Option<WorldProperties> {
        let conn = database.get_connection();
        let mut stmt = conn
            .prepare("SELECT data FROM storage WHERE name = ?")
            .unwrap();

        let data: String = match stmt.query_row(["world_properties"], |row| row.get(0)) {
            Ok(data) => data,
            Err(_) => return None,
        };

        let properties: WorldProperties = serde_json::from_str(&data).unwrap();
        return Some(properties);
    }

    fn save(&self, database: Res<Database>) {
        let conn = database.get_connection();
        let mut stmt = conn
            .prepare("INSERT OR REPLACE INTO storage (name, data) VALUES (?,?)")
            .unwrap();

        stmt.execute(rusqlite::params![
            "world_properties",
            serde_json::to_string(self).unwrap()
        ])
        .unwrap();
    }
}

/// The default spawn point, as opposed to the unique spawn point of a player.
#[derive(Default, Serialize, Deserialize)]
pub struct SpawnPoint {
    pub center: IVec3,
    pub radius: i32,
}
