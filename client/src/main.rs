use bevy::{
    audio::{AudioPlugin, SpatialScale},
    // diagnostic::{FrameTimeDiagnosticsPlugin, LogDiagnosticsPlugin},
    prelude::*,
    window::WindowFocused,
};

mod assets;
mod audio;
mod cli;
mod game_state;
mod modding;
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
    settings::initialize();

    if cli::parse() {
        return;
    }

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
                    default_spatial_scale: SpatialScale::new(0.5),
                    ..default()
                }),
        )
        // .add_plugins(LogDiagnosticsPlugin::default())
        // .add_plugins(FrameTimeDiagnosticsPlugin::default())
        .add_plugins(settings::SettingsPlugin)
        .add_plugins(networking::ClientPlugin)
        .add_plugins(assets::AssetPlugin)
        .add_plugins(audio::AudioPlugin)
        .add_plugins(particles::ParticlePlugin)
        .add_plugins(game_state::GameStatePlugin)
        .add_plugins(rendering::RenderingPlugin)
        .add_plugins(player::PlayerPlugin)
        .add_plugins(world::WorldPlugin)
        .add_plugins(ui::UiPlugin)
        .add_plugins(singleplayer::SinglePlayerPlugin)
        .add_systems(Update, fix_keys_not_released_on_focus_loss)
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
