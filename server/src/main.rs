use fmc::bevy::{
    //diagnostic::{FrameTimeDiagnosticsPlugin, LogDiagnosticsPlugin},
    prelude::*,
};

mod assets;
mod items;
mod mobs;
mod players;
mod settings;
mod skybox;
mod world;

fn main() {
    App::new()
        .insert_resource(settings::Settings::load())
        .add_plugins(fmc::DefaultPlugins)
        //.add_plugins((FrameTimeDiagnosticsPlugin, FrameCountPlugin))
        .add_plugins(assets::AssetPlugin)
        .add_plugins(items::ItemPlugin)
        .add_plugins(players::PlayerPlugin)
        .add_plugins(world::WorldPlugin)
        .add_plugins(skybox::SkyPlugin)
        .add_plugins(mobs::MobsPlugin)
        .run();
}
