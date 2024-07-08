use std::net::SocketAddr;

use bevy::prelude::*;
use fmc_networking::{messages, ConnectionId, NetworkServer, ServerNetworkEvent};

use crate::{blocks::Blocks, items::Items, models::Models, world::RenderDistance};

// TODO: I stripped this for most of its functionality, and it's a little too lean now. Move server
// setup to main, and sending the server config to fmc_networking::server
pub struct ServerPlugin;
impl Plugin for ServerPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(fmc_networking::ServerPlugin)
            .add_systems(PostStartup, server_setup)
            .add_systems(Update, handle_network_events);
    }
}

fn server_setup(mut net: ResMut<NetworkServer>) {
    let socket_address: SocketAddr = "127.0.0.1:42069".parse().unwrap();

    net.listen(socket_address);

    info!("Started listening for new connections!");
}

fn handle_network_events(
    net: Res<NetworkServer>,
    render_distance: Res<RenderDistance>,
    assets_hash: Res<crate::assets::AssetArchiveHash>,
    models: Res<Models>,
    items: Res<Items>,
    connection_query: Query<&ConnectionId>,
    mut network_events: EventReader<ServerNetworkEvent>,
) {
    for event in network_events.read() {
        match event {
            ServerNetworkEvent::Connected { entity, .. } => {
                let connection_id = connection_query.get(*entity).unwrap();
                net.send_one(
                    *connection_id,
                    messages::ServerConfig {
                        assets_hash: assets_hash.hash.clone(),
                        block_ids: Blocks::get().asset_ids(),
                        model_ids: models.asset_ids(),
                        item_ids: items.asset_ids(),
                        render_distance: render_distance.chunks,
                    },
                );
            }
            _ => {}
        }
    }
}
