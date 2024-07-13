use bevy::{prelude::*, window::WindowFocused};
use fmc_networking::ClientNetworkEvent;

pub struct GameStatePlugin;
impl Plugin for GameStatePlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<GameState>();
        app.add_systems(Update, (pause_when_unfocused, connection_change));
    }
}

/// The overarching states the game can be in.
#[derive(States, PartialEq, Eq, Debug, Clone, Hash, Default)]
pub enum GameState {
    #[default]
    Launcher,
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

fn connection_change(
    mut network_events: EventReader<ClientNetworkEvent>,
    game_state: Res<State<GameState>>,
    mut next_game_state: ResMut<NextState<GameState>>,
) {
    for event in network_events.read() {
        match event {
            ClientNetworkEvent::Disconnected(_) | ClientNetworkEvent::Error(_) => {
                if *game_state.get() != GameState::Connecting {
                    next_game_state.set(GameState::Launcher);
                }
            }
            _ => (),
        }
    }
}
