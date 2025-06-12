use bevy::{color::palettes::css::DARK_GRAY, prelude::*, window::WindowFocused};

use super::{GuiState, Interface, Interfaces};
use crate::{game_state::GameState, networking::NetworkClient, ui::client::widgets::*};

pub struct PauseMenuPlugin;
impl Plugin for PauseMenuPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup).add_systems(
            Update,
            (
                (resume_button, quit_button, escape_key).run_if(in_state(GuiState::PauseMenu)),
                (pause_when_unfocused).run_if(in_state(GameState::Playing)),
            ),
        );
    }
}

#[derive(Component)]
struct ResumeButton;

#[derive(Component)]
struct QuitButton;

fn setup(mut commands: Commands, mut interfaces: ResMut<Interfaces>) {
    let entity = commands
        .spawn((
            Interface,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                row_gap: Val::Px(4.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor::from(DARK_GRAY.with_alpha(0.5)),
        ))
        .with_children(|parent| {
            parent
                .spawn(Node {
                    width: Val::Px(200.0),
                    height: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    justify_content: JustifyContent::Center,
                    //align_items: AlignItems::Center,
                    ..default()
                })
                .with_children(|parent| {
                    parent
                        .spawn_button("Resume", Srgba::gray(0.7))
                        .insert(ResumeButton);
                    parent
                        .spawn_button("Quit", Srgba::gray(0.7))
                        .insert(QuitButton);
                });
        })
        .id();
    interfaces.insert(GuiState::PauseMenu, entity);
}

fn quit_button(
    net: Res<NetworkClient>,
    mut gui_state: ResMut<NextState<GuiState>>,
    button_query: Query<&Interaction, (Changed<Interaction>, With<QuitButton>)>,
) {
    if let Ok(interaction) = button_query.get_single() {
        if *interaction == Interaction::Pressed {
            gui_state.set(GuiState::MainMenu);
            net.disconnect("");
        }
    }
}

fn resume_button(
    mut gui_state: ResMut<NextState<GuiState>>,
    button_query: Query<&Interaction, (Changed<Interaction>, With<ResumeButton>)>,
) {
    if let Ok(interaction) = button_query.get_single() {
        if *interaction == Interaction::Pressed {
            gui_state.set(GuiState::None);
        }
    }
}

fn escape_key(
    gui_state: Res<State<GuiState>>,
    mut next_gui_state: ResMut<NextState<GuiState>>,
    input: Res<ButtonInput<KeyCode>>,
) {
    if input.just_pressed(KeyCode::Escape) && *gui_state.get() == GuiState::PauseMenu {
        next_gui_state.set(GuiState::None);
    }
}

// TODO: If the client was paused by being unfocused it should unpause when focused again.
fn pause_when_unfocused(
    mut gui_state: ResMut<NextState<GuiState>>,
    mut focus_events: EventReader<WindowFocused>,
) {
    for event in focus_events.read() {
        if !event.focused {
            gui_state.set(GuiState::PauseMenu);
        }
    }
}
