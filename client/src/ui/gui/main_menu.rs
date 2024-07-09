use bevy::prelude::*;

use super::{InterfaceBundle, Interfaces, UiState};
use crate::{game_state::GameState, ui::widgets::*};

pub struct MainMenuPlugin;
impl Plugin for MainMenuPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup).add_systems(
            Update,
            (press_singleplayer, press_multiplayer).run_if(in_state(UiState::MainMenu)),
        );
    }
}

#[derive(Component)]
struct SinglePlayerButton;
#[derive(Component)]
struct MultiPlayerButton;

fn setup(mut commands: Commands, mut interfaces: ResMut<Interfaces>) {
    let entity = commands
        .spawn(InterfaceBundle {
            background_color: Color::DARK_GRAY.into(),
            style: Style {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                row_gap: Val::Px(4.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            ..default()
        })
        .with_children(|parent| {
            // Singleplayer button
            parent
                .spawn_button(200.0, "Singleplayer")
                .insert(SinglePlayerButton);
            parent
                .spawn_button(200.0, "Multiplayer")
                .insert(MultiPlayerButton);
        })
        .id();
    interfaces.insert(UiState::MainMenu, entity);
}

fn press_singleplayer(
    mut net: ResMut<fmc_networking::NetworkClient>,
    button_query: Query<&Interaction, (Changed<Interaction>, With<SinglePlayerButton>)>,
    mut game_state: ResMut<NextState<GameState>>,
    mut server_process: Local<Option<std::process::Child>>,
) {
    if let Ok(interaction) = button_query.get_single() {
        if *interaction == Interaction::Pressed {
            if let Some(mut child) = server_process.take() {
                // TODO: Manage the server process properly, it currently runs in the back when you
                // disconnect, should stop immediately.
                child.kill().ok();
            }

            let path = String::from("fmc_server/server") + std::env::consts::EXE_EXTENSION;
            match std::process::Command::new(&std::fs::canonicalize(path).unwrap())
                .current_dir("fmc_server")
                .spawn()
            {
                Err(e) => {
                    error!("Failed to start server, error: {e}");
                    return;
                }
                Ok(c) => *server_process = Some(c),
            };

            net.connect("127.0.0.1:42069");
            game_state.set(GameState::Connecting);
        }
    }
}

fn press_multiplayer(
    mut ui_state: ResMut<NextState<UiState>>,
    button_query: Query<&Interaction, (Changed<Interaction>, With<MultiPlayerButton>)>,
) {
    if let Ok(interaction) = button_query.get_single() {
        if *interaction == Interaction::Pressed {
            ui_state.set(UiState::MultiPlayer);
        }
    }
}
