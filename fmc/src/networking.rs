use std::net::SocketAddr;

use bevy::prelude::*;
use fmc_networking::{messages, NetworkServer};

use crate::{blocks::Blocks, items::Items, models::Models, world::RenderDistance};

// TODO: I stripped this for most of its functionality, and it's a little too lean now. Move server
// setup to main, and sending the server config to fmc_networking::server
pub struct ServerPlugin;
impl Plugin for ServerPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(fmc_networking::ServerPlugin)
            .add_systems(Startup, server_setup);
    }
}

fn server_setup(
    mut net: ResMut<NetworkServer>,
    render_distance: Res<RenderDistance>,
    assets_hash: Res<crate::assets::AssetArchiveHash>,
    models: Res<Models>,
    blocks: Res<Blocks>,
    items: Res<Items>,
) {
    let socket_address: SocketAddr = "127.0.0.1:42069".parse().unwrap();

    net.start(
        socket_address,
        messages::ServerConfig {
            assets_hash: assets_hash.hash.clone(),
            block_ids: blocks.asset_ids(),
            model_ids: models.asset_ids(),
            item_ids: items.asset_ids(),
            render_distance: render_distance.chunks,
        },
    );

    info!("Started listening for new connections!");
}
