use bevy::prelude::*;

use super::{InterfaceBundle, Interfaces, UiState};
use crate::{networking::Identity, ui::widgets::*};

pub struct LoginPlugin;
impl Plugin for LoginPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup)
            .add_systems(Update, press_play.run_if(in_state(UiState::Login)));
    }
}

#[derive(Component)]
struct LoginButton;

#[derive(Component)]
struct Username;

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
            parent
                .spawn(NodeBundle {
                    style: Style {
                        width: Val::Percent(100.0),
                        height: Val::Px(12.0),
                        justify_content: JustifyContent::Center,
                        ..default()
                    },
                    ..default()
                })
                .with_children(|parent| {
                    parent.spawn_text(
                        "Enter username:",
                        12.0,
                        Color::WHITE,
                        FlexDirection::Row,
                        JustifyContent::Center,
                        AlignItems::Center,
                    );
                });
            parent.spawn_textbox(41.5, "").insert(Username);
            parent.spawn_button(200.0, "Play").insert(LoginButton);
        })
        .id();
    interfaces.insert(UiState::Login, entity);
}

fn press_play(
    mut ui_state: ResMut<NextState<UiState>>,
    mut identity: ResMut<Identity>,
    username: Query<&TextBox, With<Username>>,
    button_query: Query<&Interaction, (Changed<Interaction>, With<LoginButton>)>,
) {
    if let Ok(interaction) = button_query.get_single() {
        identity.username = username.single().text.clone();
        if identity.username.is_empty() {
            return;
        }

        std::fs::write("./identity.txt", &identity.username).ok();

        if *interaction == Interaction::Pressed {
            ui_state.set(UiState::MainMenu);
        }
    }
}
