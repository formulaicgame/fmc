use bevy::prelude::*;

use fmc_protocol::messages;

use crate::{game_state::GameState, networking::NetworkClient};

pub(super) struct SettingsPlugin;
impl Plugin for SettingsPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Settings::load()).add_systems(
            Update,
            set_render_distance.run_if(in_state(GameState::Playing)),
        );
    }
}

// TODO: Serialization for better saving/loading? Easy to forget to add a field.
// I don't think serde supports an easy way to fall back to the default on invalid value without
// writing a custom fallback function for each value.
// This makes it useless probably. It needs to replace invalids and then write the result to the
// file again to resolve. To avoid having to notify the user through the program.
#[derive(Resource)]
pub struct Settings {
    /// Render distance in chunks
    pub render_distance: u32,
    /// Field of view of camera
    pub fov: f32,
    /// Sound volume
    pub volume: f32,
    /// Mouse sensitivity
    pub sensitivity: f32,
    /// Horizontal speed while flying
    pub flight_speed: f32,
    /// Fog that limits visibility
    pub fog: DistanceFog,
}

impl Settings {
    fn load() -> Self {
        //let path = dirs::config_dir().unwrap().join("fmc/config.txt");
        let settings = Settings::default();

        return settings;
    }
    //fn save(&self) {
    //    let mut contents = "".to_owned()
    //    contents += "render_distance = " + &self.render_distance.to_string() + "\n"
    //    contents += "fov = " + &self.fov.to_string();
    //
    //}
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            render_distance: 16,
            fov: std::f32::consts::PI / 3.0,
            volume: 1.0,
            sensitivity: 0.00005,
            flight_speed: 50.0,
            fog: DistanceFog {
                color: Color::NONE,
                ..default()
            },
        }
    }
}

//fn save_settings(
//    settings: Res<Settings>
//) {
//    // Writes on addition too to remove any invalid values that might have been introduced by the
//    //user.
//    if settings.is_changed() {
//        settings.save();
//    }
//}

fn set_render_distance(
    net: Res<NetworkClient>,
    server_config: Res<messages::ServerConfig>,
    mut settings: ResMut<Settings>,
) {
    if server_config.is_changed() {
        settings.render_distance = settings.render_distance.min(server_config.render_distance);
        net.send_message(messages::RenderDistance {
            chunks: settings.render_distance,
        });
    }
}
