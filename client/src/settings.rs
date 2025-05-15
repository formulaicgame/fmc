use std::{
    io::{prelude::*, BufReader},
    path::PathBuf,
};

use bevy::prelude::*;

use fmc_protocol::messages;

use crate::{game_state::GameState, networking::NetworkClient};

pub(super) struct SettingsPlugin;
impl Plugin for SettingsPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Settings::load()).add_systems(
            Update,
            (
                save_settings.run_if(resource_changed::<Settings>),
                set_render_distance.run_if(in_state(GameState::Playing)),
            ),
        );
    }
}

pub fn initialize() {
    let settings = Settings::load();

    if !settings.config_dir().exists() {
        std::fs::create_dir(settings.config_dir()).unwrap();
    }

    if !settings.data_dir().exists() {
        std::fs::create_dir(settings.data_dir()).unwrap();
    }
}

// TODO: Serialization for better saving/loading? Easy to forget to add a field.
// I don't think serde supports an easy way to fall back to the default on invalid value without
// writing a custom fallback function for each value.
// This makes it useless probably. It needs to replace invalids and then write the result to the
// file again to resolve. To avoid having to notify the user through the program.
#[derive(Resource, Debug)]
pub struct Settings {
    /// Render distance in chunks
    pub render_distance: u32,
    /// Field of view of camera
    pub fov: f32,
    /// Sound volume
    pub volume: f32,
    /// Mouse sensitivity
    pub sensitivity: f32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            render_distance: 16,
            fov: std::f32::consts::PI / 3.0,
            volume: 1.0,
            sensitivity: 0.00005,
        }
    }
}

impl Settings {
    // TODO: This one should be definable in the settings file.
    pub fn data_dir(&self) -> PathBuf {
        dirs::data_dir()
            .expect("Missing data directory")
            .join("fmc")
    }

    pub fn config_dir(&self) -> PathBuf {
        dirs::config_dir()
            .expect("Missing configuration directory")
            .join("fmc")
    }

    fn config_file(&self) -> PathBuf {
        self.config_dir().join("settings.ini")
    }

    fn load() -> Self {
        let mut settings = Settings::default();

        let path = settings.config_file();
        let file = match std::fs::File::open(&path) {
            Ok(f) => f,
            Err(e) => {
                if e.kind() == std::io::ErrorKind::NotFound {
                    settings.save();
                } else {
                    error!("Error reading {}: {}", path.display(), e);
                }
                return settings;
            }
        };
        let reader = BufReader::new(file);

        for (line_num, line) in reader.lines().enumerate() {
            let line = line.unwrap();

            // comments
            if line.starts_with("#") {
                continue;
            }

            let Some((name, value)) = line.split_once("=") else {
                error!(
                    "Error reading settings.ini at line {}. All settings must be of the format 'name = setting', it cannot be '{}'",
                    line_num, line
                );
                continue;
            };

            let name = name.trim();
            let value = value.trim();

            let err = |property: &str, expected: &str, value: &str| {
                error!(
                    "The '{}' setting must be a {}, cannot be '{}'",
                    property, expected, value
                );
            };
            match name {
                "render-distance" => {
                    if let Ok(value) = value.parse::<u32>() {
                        settings.render_distance = value;
                    } else {
                        err("render-distance", "number", value);
                    }
                }
                "fov" => {
                    if let Ok(value) = value.parse::<f32>() {
                        settings.fov = value;
                    } else {
                        err("fov", "number", value);
                    }
                }
                "volume" => {
                    if let Ok(value) = value.parse::<f32>() {
                        settings.volume = value.min(1.0).max(0.0);
                    } else {
                        err("volume", "number", value);
                    }
                }
                "sensitivity" => {
                    if let Ok(value) = value.parse::<f32>() {
                        settings.sensitivity = value.min(1.0).max(0.0);
                    } else {
                        err("sensitivity", "number", value);
                    }
                }
                _ => error!("Invalid setting '{name}' in settings.ini at line {line_num}"),
            }
        }

        return settings;
    }

    #[rustfmt::skip]
    fn save(&self) {
        let contents = String::new()
            + "render-distance = " + &self.render_distance.to_string() + "\n"
            + "fov = " + &self.fov.to_string() + "\n"
            + "volume = " + &self.volume.to_string() + "\n"
            + "sensitivity = " + &self.sensitivity.to_string() + "\n";

        if let Err(e) = std::fs::write(self.config_file(), contents) {
            error!("Failed to write config file: {}", e);
        }
    }
}

fn save_settings(settings: Res<Settings>) {
    settings.save();
}

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
