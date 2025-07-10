use std::{
    path::PathBuf,
    process::{Child, Stdio},
};

use bevy::prelude::*;
use fmc_protocol::messages;

use crate::{
    game_state::GameState,
    networking::{ConnectionEvent, NetworkClient},
};

pub struct SinglePlayerPlugin;
impl Plugin for SinglePlayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<LaunchSinglePlayer>()
            .insert_resource(ServerProcess(None))
            .add_systems(
                Update,
                (
                    launch_singleplayer_server,
                    kill_server_on_disconnect.run_if(on_event::<messages::Disconnect>),
                ),
            );
    }
}

#[derive(Event)]
pub struct LaunchSinglePlayer {
    pub path: PathBuf,
}

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
    mut net: ResMut<NetworkClient>,
    mut server_process: ResMut<ServerProcess>,
    mut launch_events: EventReader<LaunchSinglePlayer>,
    mut connection_events: EventWriter<ConnectionEvent>,
) {
    for _ in launch_events.read() {
        if server_process.0.is_some() {
            return;
        }

        let path = String::from("fmc_server/server") + std::env::consts::EXE_SUFFIX;

        if !std::path::Path::new(&path).exists() {
            info!("Still downloading server executable");
            return;
        }

        info!("Starting single player server");
        match std::process::Command::new(&std::fs::canonicalize(path).unwrap())
            .current_dir("fmc_server")
            // The server listens for this in order to organize its files differently when running
            // as a cargo project. We don't want that when running it through the client.
            .env_remove("CARGO")
            .stdin(Stdio::piped())
            .spawn()
        {
            Err(e) => {
                error!("Failed to start server, error: {e}");
                return;
            }
            Ok(c) => *server_process = ServerProcess(Some(c)),
        };

        connection_events.write(ConnectionEvent {
            address: "127.0.0.1".to_owned(),
        });
    }
}

fn kill_server_on_disconnect(mut server_process: ResMut<ServerProcess>) {
    if let Some(mut process) = server_process.0.take() {
        process.kill().ok();
    }
}
