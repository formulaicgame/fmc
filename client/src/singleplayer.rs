use std::process::{Child, Stdio};

use bevy::prelude::*;
use fmc_protocol::messages;

use crate::{game_state::GameState, networking::NetworkClient};

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
    mut net: ResMut<NetworkClient>,
    mut game_state: ResMut<NextState<GameState>>,
    mut server_process: ResMut<ServerProcess>,
    mut launch_events: EventReader<LaunchSinglePlayer>,
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
            // as a cargo project. We don't want it to do this when the client is run as a cargo
            // project.
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

        // TODO: Despite the connect function having a timeout it will still instantly return
        // "connection refused" while the server is starting up. How do you go about waiting for it
        // to finish startup?
        std::thread::sleep(std::time::Duration::from_secs(2));

        net.connect("127.0.0.1:42069".parse().unwrap());
        game_state.set(GameState::Connecting);
    }
}

fn kill_server_on_disconnect(mut server_process: ResMut<ServerProcess>) {
    if let Some(mut process) = server_process.0.take() {
        process.kill().ok();
    }
}
