use std::process::{Child, Stdio};

use bevy::{prelude::*, tasks::AsyncComputeTaskPool};
use fmc_protocol::messages;

use crate::{game_state::GameState, networking::NetworkClient};

pub struct SinglePlayerPlugin;
impl Plugin for SinglePlayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<LaunchSinglePlayer>()
            .insert_resource(ServerProcess(None))
            .add_systems(Startup, download_game)
            .add_systems(
                Update,
                (
                    launch_singleplayer_server,
                    kill_server_on_disconnect.run_if(on_event::<messages::Disconnect>()),
                ),
            );
    }
}

// Temporary hard link to game until proper game hub
fn download_game() {
    let server_path = String::from("fmc_server/server") + std::env::consts::EXE_EXTENSION;
    if std::path::Path::new(&server_path).exists() {
        return;
    }

    AsyncComputeTaskPool::get().spawn(async {
        let url = match (std::env::consts::OS, std::env::consts::ARCH) {
            ("linux", "x86_64") => "https://github.com/formulaicgame/fmc_vanilla/releases/download/nightly/x86_64-unknown-linux-gnu",
            ("windows", "x86_64") => "https://github.com/formulaicgame/fmc_vanilla/releases/download/nightly/x86_64-pc-windows-msvc.exe",
            ("macos", "x86_64") => "https://github.com/formulaicgame/fmc_vanilla/releases/download/nightly/x86_64-apple-darwin",
            ("macos", "aarch64") => "https://github.com/formulaicgame/fmc_vanilla/releases/download/nightly/aarch64-apple-darwin",
            _ => return
        };
        let response = match reqwest::blocking::get(url) {
            Ok(r) => r,
            Err(_) => return
        };
        let bytes = match response.bytes() {
            Ok(b) => b,
            Err(_) => return
        };

        std::fs::create_dir("fmc_server").ok();
        let path = String::from("fmc_server/server") + std::env::consts::EXE_EXTENSION;
        std::fs::write(&path, bytes).ok();

        if std::env::consts::FAMILY == "unix" {
            std::process::Command::new("chmod").arg("+x").arg(&path).output().ok();
        }
    }).detach();
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
        let path = String::from("fmc_server/server") + std::env::consts::EXE_EXTENSION;

        if !std::path::Path::new(&path).exists() {
            info!("Still downloading server executable");
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
