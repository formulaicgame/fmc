use bevy::prelude::*;

// Uses cargo to build servers from scratch through a simple configuration file. Allows the client
// to build servers with the server mods they want.
pub mod server_builder;
// The server can define wasm mods that can affect the client through a limited interface.
mod wasm;

pub struct ModPlugin;
impl Plugin for ModPlugin {
    fn build(&self, app: &mut App) {}
}
