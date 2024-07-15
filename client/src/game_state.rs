use bevy::prelude::*;
use fmc_networking::ClientNetworkEvent;

pub struct GameStatePlugin;
impl Plugin for GameStatePlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<GameState>();
        app.add_systems(Update, on_disconnect);
    }
}

/// The overarching states the game can be in.
#[derive(States, PartialEq, Eq, Debug, Clone, Hash, Default)]
pub enum GameState {
    #[default]
    Launcher,
    Connecting,
    Playing,
}

fn on_disconnect(
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
