use bevy::prelude::*;

use super::{InterfaceBundle, Interfaces, UiState};
use crate::{game_state::GameState, ui::widgets::*};

pub struct PauseMenuPlugin;
impl Plugin for PauseMenuPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup).add_systems(
            Update,
            (resume_button, quit_button, escape_key).run_if(in_state(UiState::PauseMenu)),
        );
    }
}

#[derive(Component)]
struct ResumeButton;

#[derive(Component)]
struct QuitButton;

fn setup(mut commands: Commands, mut interfaces: ResMut<Interfaces>) {
    let entity = commands
        .spawn(InterfaceBundle {
            background_color: Color::DARK_GRAY.with_a(0.5).into(),
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
            parent.spawn_button(200.0, "Resume").insert(ResumeButton);
            parent.spawn_button(200.0, "Quit").insert(QuitButton);
        })
        .id();
    interfaces.insert(UiState::PauseMenu, entity);
}

fn quit_button(
    mut game_state: ResMut<NextState<GameState>>,
    button_query: Query<&Interaction, (Changed<Interaction>, With<QuitButton>)>,
) {
    if let Ok(interaction) = button_query.get_single() {
        if *interaction == Interaction::Pressed {
            game_state.set(GameState::MainMenu);
        }
    }
}

fn resume_button(
    mut game_state: ResMut<NextState<GameState>>,
    button_query: Query<&Interaction, (Changed<Interaction>, With<ResumeButton>)>,
) {
    if let Ok(interaction) = button_query.get_single() {
        if *interaction == Interaction::Pressed {
            game_state.set(GameState::Playing);
        }
    }
}

fn escape_key(mut game_state: ResMut<NextState<GameState>>, input: Res<ButtonInput<KeyCode>>) {
    if input.just_pressed(KeyCode::Escape) {
        game_state.set(GameState::Playing);
    }
}
