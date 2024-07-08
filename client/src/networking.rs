use bevy::prelude::*;
use fmc_networking::{messages, ClientNetworkEvent, NetworkClient, NetworkData};

use crate::game_state::GameState;

pub struct ClientPlugin;

// This is set during login, but is otherwise empty
#[derive(Resource, Default)]
pub struct Identity {
    pub username: String,
}

impl Plugin for ClientPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(fmc_networking::ClientPlugin)
            .add_systems(Startup, setup)
            .add_systems(
                PreUpdate,
                (
                    handle_connection,
                    handle_server_config,
                    handle_disconnect_messages,
                ),
            );
    }
}

fn setup(mut commands: Commands) {
    if let Ok(username) = std::fs::read_to_string("./identity.txt") {
        commands.insert_resource(Identity {
            username: username.trim().to_owned(),
        });
    } else {
        commands.insert_resource(Identity::default());
    }
}

// TODO: Disconnect and error message should be shown to player through the ui.
fn handle_connection(
    net: Res<NetworkClient>,
    identity: Res<Identity>,
    mut network_events: EventReader<ClientNetworkEvent>,
    mut game_state: ResMut<NextState<GameState>>,
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
                game_state.set(GameState::MainMenu);
                info!("Disconnected from server");
            }
            ClientNetworkEvent::Error(err) => {
                game_state.set(GameState::MainMenu);
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
