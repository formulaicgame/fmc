use bevy::prelude::*;

use crate::assets::AssetState;

pub struct GameStatePlugin;
impl Plugin for GameStatePlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<GameState>()
            .add_systems(OnExit(AssetState::Loading), start_playing);
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

fn start_playing(mut game_state: ResMut<NextState<GameState>>) {
    game_state.set(GameState::Playing);
}
