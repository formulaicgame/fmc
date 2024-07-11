use std::process::{Child, Stdio};

use bevy::prelude::*;
use fmc_networking::ClientNetworkEvent;

use crate::game_state::GameState;

pub struct SinglePlayerPlugin;
impl Plugin for SinglePlayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<LaunchSinglePlayer>()
            .insert_resource(ServerProcess(None))
            .add_systems(
                Update,
                (launch_singleplayer_server, kill_server_on_disconnect),
            );
    }
}

#[derive(Event)]
pub struct LaunchSinglePlayer {}

#[derive(Resource)]
struct ServerProcess(Option<Child>);

// TODO: If sigkill is issued I don't think this is enough
impl Drop for ServerProcess {
    fn drop(&mut self) {
        if let Some(mut server) = self.0.take() {
            server.kill().ok();
        }
    }
}

fn launch_singleplayer_server(
    mut net: ResMut<fmc_networking::NetworkClient>,
    mut game_state: ResMut<NextState<GameState>>,
    mut server_process: ResMut<ServerProcess>,
    mut launch_events: EventReader<LaunchSinglePlayer>,
) {
    for _ in launch_events.read() {
        let path = String::from("fmc_server/server") + std::env::consts::EXE_EXTENSION;

        if !std::path::Path::new(&path).exists() {
            return;
        }

        info!("Starting single player server");
        match std::process::Command::new(&std::fs::canonicalize(path).unwrap())
            .current_dir("fmc_server")
            .stdin(Stdio::piped())
            .spawn()
        {
            Err(e) => {
                error!("Failed to start server, error: {e}");
                return;
            }
            Ok(c) => *server_process = ServerProcess(Some(c)),
        };

        net.connect("127.0.0.1:42069");
        game_state.set(GameState::Connecting);
    }
}

fn kill_server_on_disconnect(
    mut network_events: EventReader<ClientNetworkEvent>,
    mut server_process: ResMut<ServerProcess>,
) {
    for event in network_events.read() {
        if let ClientNetworkEvent::Disconnected(_) = event {
            if let Some(mut process) = server_process.0.take() {
                process.kill().ok();
            }
        }
    }
}
