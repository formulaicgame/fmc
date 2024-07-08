use bevy::{prelude::*, window::WindowFocused};
use fmc_networking::{messages, NetworkClient};

use crate::assets::AssetState;

pub struct GameStatePlugin;
impl Plugin for GameStatePlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<GameState>();
        app.add_systems(Update, pause_when_unfocused)
            .add_systems(OnExit(AssetState::Loading), finished_loading_start_game);
    }
}

/// The overarching states the game can be in.
#[derive(States, PartialEq, Eq, Debug, Clone, Hash, Default)]
pub enum GameState {
    #[default]
    MainMenu,
    Connecting,
    Playing,
    Paused,
}

impl GameState {
    pub fn in_game(state: Res<State<GameState>>) -> bool {
        match state.get() {
            GameState::Playing | GameState::Paused => true,
            _ => false,
        }
    }
}

// All assets are loaded, it can now start the main game loop
fn finished_loading_start_game(net: Res<NetworkClient>, mut state: ResMut<NextState<GameState>>) {
    net.send_message(messages::ClientFinishedLoading);
    state.set(GameState::Playing);
}

// TODO: If the client was paused by being unfocused it should unpause when focused again.
fn pause_when_unfocused(
    state: Res<State<GameState>>,
    mut next_state: ResMut<NextState<GameState>>,
    mut focus_events: EventReader<WindowFocused>,
) {
    for event in focus_events.read() {
        if *state.get() == GameState::Playing {
            if !event.focused {
                next_state.set(GameState::Paused);
            }
        }
    }
}
