use bevy::prelude::*;

use fmc_networking::{messages, NetworkClient, NetworkData};

pub(super) struct SettingsPlugin;
impl Plugin for SettingsPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Settings::load())
            .add_systems(Update, set_render_distance);
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
    pub fog: FogSettings,
}

impl Settings {
    fn load() -> Self {
        //let path = dirs::config_dir().unwrap().join("fmc/config.txt");
        let settings = Settings::default();

        return settings;
    }
    //fn save(&self) {
    //    let contents = "".to_owned()
    //    + "render_distance = " + &self.render_distance.to_string() + "\n"
    //    + "fov = " + &self.fov.to_string();
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
            fog: FogSettings {
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

// TODO: This is a placeholder since there's no settings menu yet.
fn set_render_distance(
    mut settings: ResMut<Settings>,
    mut server_config_events: EventReader<NetworkData<messages::ServerConfig>>,
) {
    for server_config in server_config_events.read() {
        settings.render_distance = settings.render_distance.min(server_config.render_distance);
    }
}
