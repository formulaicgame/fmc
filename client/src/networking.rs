use bevy::prelude::*;
use fmc_networking::{messages, ClientNetworkEvent, NetworkClient, NetworkData};

use crate::game_state::GameState;

pub struct ClientPlugin;

impl Plugin for ClientPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Identity::read_from_file())
            .add_plugins(fmc_networking::ClientPlugin)
            .add_systems(
                PreUpdate,
                (
                    handle_connection,
                    handle_server_config.run_if(in_state(GameState::Connecting)),
                    handle_disconnect_messages,
                ),
            );
    }
}

#[derive(Resource)]
pub struct Identity {
    pub username: String,
}

impl Identity {
    fn read_from_file() -> Self {
        if let Ok(username) = std::fs::read_to_string("./identity.txt") {
            Identity {
                username: username.trim().to_owned(),
            }
        } else {
            Identity {
                username: String::new(),
            }
        }
    }

    pub fn is_valid(&self) -> bool {
        !self.username.is_empty()
    }
}

// TODO: Disconnect and error message should be shown to player through the ui.
fn handle_connection(
    net: Res<NetworkClient>,
    identity: Res<Identity>,
    mut network_events: EventReader<ClientNetworkEvent>,
) {
    for event in network_events.read() {
        match event {
            ClientNetworkEvent::Connected => {
                net.send_message(messages::ClientIdentification {
                    name: identity.username.clone(),
                });
                info!("Connected to server");
            }
            ClientNetworkEvent::Disconnected(_message) => {
                info!("Disconnected from server");
            }
            ClientNetworkEvent::Error(err) => {
                error!("{}", err);
            }
        }
    }
}

fn handle_server_config(
    mut commands: Commands,
    mut server_config_events: EventReader<NetworkData<messages::ServerConfig>>,
) {
    for event in server_config_events.read() {
        let server_config: messages::ServerConfig = (*event).clone();
        commands.insert_resource(server_config);
    }
}

fn handle_disconnect_messages(
    mut disconnect_events: EventReader<NetworkData<messages::Disconnect>>,
) {
    for event in disconnect_events.read() {
        error!("{}", &event.message);
    }
}
