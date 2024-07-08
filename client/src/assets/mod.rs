use sha1::Digest;
use std::io::prelude::*;

use bevy::prelude::*;
use fmc_networking::{messages, NetworkData};

mod block_textures;
mod materials;
pub mod models;

pub use block_textures::BlockTextures;
pub use materials::Materials;

/// Assets are downloaded on connection to the server. It first waits for the server config. Then
/// checks if server_config.asset_hash is the same as the hash of any stored assets. If not it asks
/// for assets from the server. It then loads them.
#[derive(States, Default, Debug, Clone, PartialEq, Eq, Hash)]
pub enum AssetState {
    #[default]
    Inactive,
    Downloading,
    Loading,
}

// TODO: Remove everything, call all functions from their modules, check for resource_added for
// dependencies. Though when it comes to the stuff that depends Blocks since that is a global.
// I want to clean this up so stuff doesn't have to be exposed, and it would be nice to have
// asset reloading where things that dependron each other are automatically redone.
pub struct AssetPlugin;
impl Plugin for AssetPlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<AssetState>()
            .add_plugins(models::ModelPlugin);

        app.add_systems(
            Update,
            (
                begin_asset_loading.run_if(in_state(AssetState::Inactive)),
                handle_assets_response.run_if(in_state(AssetState::Downloading)),
            ),
        )
        .add_systems(
            OnEnter(AssetState::Loading),
            (
                block_textures::load_block_textures,
                models::load_models,
                crate::ui::server::key_bindings::load_key_bindings,
                apply_deferred,
                materials::load_materials,
                apply_deferred,
                crate::world::blocks::load_blocks,
                apply_deferred,
                crate::ui::server::items::load_items,
                crate::ui::server::load_interfaces,
                finish,
            )
                .chain(),
        );
    }
}

fn finish(mut asset_state: ResMut<NextState<AssetState>>) {
    asset_state.set(AssetState::Inactive);
}

// TODO: The server can crash the client by sending multiple server configs. Need proper cleanup of
// state between connections, and then just listen for when serverconfig is added as a resource.
fn begin_asset_loading(
    net: Res<fmc_networking::NetworkClient>,
    mut server_config_event: EventReader<NetworkData<messages::ServerConfig>>,
    mut asset_state: ResMut<NextState<AssetState>>,
) {
    for config in server_config_event.read() {
        if !has_assets(&config.assets_hash) {
            info!("Downloading assets from the server...");
            net.send_message(messages::AssetRequest);
            asset_state.set(AssetState::Downloading)
        } else {
            asset_state.set(AssetState::Loading)
        }
    }
}

fn handle_assets_response(
    mut asset_state: ResMut<NextState<AssetState>>,
    mut asset_events: EventReader<NetworkData<messages::AssetResponse>>,
) {
    // TODO: Does this need an explicit timeout? Don't want to let the server be able to leave the
    // client in limbo without the player being able to quit.
    // TODO: Unpacking stores tarball in extraction directory, delete it.
    for tarball in asset_events.read() {
        info!("Loading assets...");
        // Remove old assets if they exist.
        std::fs::remove_dir_all("server_assets").ok();

        let mut archive = tar::Archive::new(std::io::Cursor::new(&tarball.file));
        archive.unpack("./server_assets").unwrap();

        // Write the hash to file to check against the next time we connect.
        let mut file = std::fs::File::create("server_assets/hash.txt").unwrap();
        file.write_all(&sha1::Sha1::digest(&tarball.file)).unwrap();
        file.flush().unwrap();

        asset_state.set(AssetState::Loading);
    }
}

fn has_assets(server_hash: &Vec<u8>) -> bool {
    let mut file = match std::fs::File::open("server_assets/hash.txt") {
        Ok(f) => f,
        Err(_) => return false,
    };
    let mut hash: Vec<u8> = Vec::new();
    file.read_to_end(&mut hash).unwrap();

    if hash != *server_hash {
        return false;
    }
    return true;
}
