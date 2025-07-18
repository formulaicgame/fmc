use std::{
    io::Write,
    path::{Path, PathBuf},
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
        app.insert_resource(SinglePlayerServer::default())
            .add_systems(
                Update,
                (start_server, kill_server).run_if(in_state(GameState::Launcher)),
            )
            .add_systems(OnEnter(GameState::Launcher), stop_server);
    }
}

#[derive(Resource, Default)]
pub struct SinglePlayerServer {
    path: Option<PathBuf>,
    process: Option<Child>,
    kill_timer: Option<Timer>,
}

impl SinglePlayerServer {
    #[track_caller]
    pub fn start(&mut self, path: impl AsRef<Path>) {
        if self.process.is_some() {
            panic!("Attempted to launch a second singleplayer server.");
        }

        self.path = Some(path.as_ref().to_path_buf());
    }
}

// Catches when the user closes the window without quitting the game.
impl Drop for SinglePlayerServer {
    fn drop(&mut self) {
        if let Some(mut process) = self.process.take() {
            let mut stdin = process.stdin.as_mut().unwrap();
            stdin.write_all(b"stop\n");

            let time = std::time::Instant::now();
            while time.elapsed().as_secs_f32() < 0.10 {
                match process.try_wait() {
                    Ok(Some(_)) => return,
                    _ => continue,
                }
            }

            process.kill().ok();
        }
    }
}

fn start_server(
    mut net: ResMut<NetworkClient>,
    mut server: ResMut<SinglePlayerServer>,
    mut connection_events: EventWriter<ConnectionEvent>,
) {
    if let Some(world_path) = server.path.take() {
        let exe_path = String::from("fmc_server/server") + std::env::consts::EXE_SUFFIX;

        if !Path::new(&exe_path).exists() {
            info!("Still downloading server executable");
            return;
        }

        info!("Starting singleplayer server");
        match std::process::Command::new(&std::fs::canonicalize(exe_path).unwrap())
            .current_dir("fmc_server")
            .arg(&world_path)
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
            Ok(c) => server.process = Some(c),
        };

        connection_events.write(ConnectionEvent {
            address: "127.0.0.1".to_owned(),
        });
    }
}

fn kill_server(time: Res<Time>, mut singleplayer_server: ResMut<SinglePlayerServer>) {
    if let Some(timer) = singleplayer_server.kill_timer.as_mut() {
        timer.tick(time.delta());

        if timer.finished() {
            error!("Couldn't stop server gracefully, killing it.");
            singleplayer_server.process.as_mut().unwrap().kill().ok();
            singleplayer_server.process = None;
            singleplayer_server.kill_timer = None;
            return;
        }

        match singleplayer_server.process.as_mut().unwrap().try_wait() {
            Ok(Some(_)) => {
                singleplayer_server.process = None;
                singleplayer_server.kill_timer = None;
            }
            _ => (),
        }
    }
}

// Try to gracefully stop the server
fn stop_server(mut singleplayer_server: ResMut<SinglePlayerServer>) {
    if let Some(process) = singleplayer_server.process.as_mut() {
        let mut stdin = process.stdin.as_mut().unwrap();
        stdin.write_all(b"stop\n");
        singleplayer_server.kill_timer = Some(Timer::new(
            std::time::Duration::from_millis(250),
            TimerMode::Once,
        ));
    }
}
