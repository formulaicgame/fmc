use bevy::{
    audio::{AudioPlugin, SpatialScale, Volume},
    diagnostic::{FrameTimeDiagnosticsPlugin, LogDiagnosticsPlugin},
    prelude::*,
    tasks::AsyncComputeTaskPool,
    window::WindowFocused,
};

mod assets;
mod audio;
mod game_state;
mod networking;
mod particles;
mod player;
mod rendering;
mod settings;
mod singleplayer;
mod ui;
mod utils;
mod world;

fn main() {
    App::new()
        //.insert_resource(Msaa { samples: 4 })
        .insert_resource(Time::<Fixed>::from_seconds(1.0 / 144.0))
        .add_plugins(
            DefaultPlugins
                .set(AssetPlugin {
                    file_path: "".to_owned(),
                    ..default()
                })
                .set(ImagePlugin::default_nearest())
                .set(AudioPlugin {
                    global_volume: GlobalVolume {
                        volume: Volume::new(1.0),
                    },
                    default_spatial_scale: SpatialScale::new(0.1),
                }),
        )
        // .add_plugins(LogDiagnosticsPlugin::default())
        // .add_plugins(FrameTimeDiagnosticsPlugin::default())
        .add_plugins(networking::ClientPlugin)
        .add_plugins(assets::AssetPlugin)
        .add_plugins(audio::AudioPlugin)
        .add_plugins(particles::ParticlePlugin)
        .add_plugins(game_state::GameStatePlugin)
        .add_plugins(rendering::RenderingPlugin)
        .add_plugins(player::PlayerPlugin)
        .add_plugins(world::WorldPlugin)
        .add_plugins(ui::UiPlugin)
        .add_plugins(settings::SettingsPlugin)
        .add_plugins(singleplayer::SinglePlayerPlugin)
        .add_systems(Update, fix_keys_not_released_on_focus_loss)
        .add_systems(Startup, download_game)
        .run();
}

// https://github.com/bevyengine/bevy/issues/4049
// https://github.com/bevyengine/bevy/issues/2068
fn fix_keys_not_released_on_focus_loss(
    mut focus_events: EventReader<WindowFocused>,
    mut key_input: ResMut<ButtonInput<KeyCode>>,
) {
    for event in focus_events.read() {
        if !event.focused {
            key_input.bypass_change_detection().release_all();
        }
    }
}

// Temporary hard link to game until proper game hub
fn download_game() {
    let server_path = String::from("fmc_server/server") + std::env::consts::EXE_EXTENSION;
    if std::path::Path::new(&server_path).exists() {
        return;
    }

    AsyncComputeTaskPool::get().spawn(async {
        let url = match (std::env::consts::OS, std::env::consts::ARCH) {
            ("linux", "x86_64") => "https://github.com/formulaicgame/fmc_vanilla/releases/download/nightly/x86_64-unknown-linux-gnu",
            ("windows", "x86_64") => "https://github.com/formulaicgame/fmc_vanilla/releases/download/nightly/x86_64-pc-windows-msvc.exe",
            ("macos", "x86_64") => "https://github.com/formulaicgame/fmc_vanilla/releases/download/nightly/x86_64-apple-darwin",
            ("macos", "aarch64") => "https://github.com/formulaicgame/fmc_vanilla/releases/download/nightly/aarch64-apple-darwin",
            _ => return
        };
        let response = match reqwest::blocking::get(url) {
            Ok(r) => r,
            Err(_) => return
        };
        let bytes = match response.bytes() {
            Ok(b) => b,
            Err(_) => return
        };

        std::fs::create_dir("fmc_server").ok();
        let path = String::from("fmc_server/server") + std::env::consts::EXE_EXTENSION;
        std::fs::write(&path, bytes).ok();

        if std::env::consts::FAMILY == "unix" {
            std::process::Command::new("chmod").arg("+x").arg(&path).output().ok();
        }
    }).detach();
}
