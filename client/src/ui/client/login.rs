use bevy::prelude::*;

use super::{GuiState, Interface, Interfaces};
use crate::{
    networking::Identity,
    settings::Settings,
    ui::{client::widgets::*, text_input::TextBox},
};

pub struct LoginPlugin;
impl Plugin for LoginPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, interface_setup)
            .add_systems(Update, press_login_button.run_if(in_state(GuiState::Login)));
    }
}

#[derive(Component)]
struct LoginButton;

#[derive(Component)]
struct Username;

fn interface_setup(mut commands: Commands, mut interfaces: ResMut<Interfaces>) {
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
            ImageNode {
                image: super::BACKGROUND,
                ..default()
            },
        ))
        .with_children(|parent| {
            parent
                .spawn(Node {
                    width: Val::Percent(30.0),
                    height: Val::Px(50.0),
                    flex_direction: FlexDirection::Column,
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    row_gap: Val::Percent(3.0),
                    ..default()
                })
                .with_children(|parent| {
                    parent.spawn_text("Enter username:");
                    parent
                        .spawn_textbox(TextBox::default().with_autofocus())
                        .insert(Username);
                    parent
                        .spawn_button(
                            "Play",
                            ButtonStyle {
                                color: Color::from(Srgba::GREEN),
                                ..default()
                            },
                        )
                        .insert(LoginButton);
                });
        })
        .id();

    interfaces.insert(GuiState::Login, entity);
}

fn press_login_button(
    mut ui_state: ResMut<NextState<GuiState>>,
    settings: Res<Settings>,
    mut identity: ResMut<Identity>,
    keys: Res<ButtonInput<KeyCode>>,
    username: Query<&TextBox, With<Username>>,
    login_button: Query<&Interaction, (Changed<Interaction>, With<LoginButton>)>,
) {
    if login_button
        .single()
        .is_ok_and(|interaction| *interaction == Interaction::Pressed)
        || keys.just_pressed(KeyCode::Enter)
    {
        let username = &username.single().unwrap().text;

        if username.is_empty() {
            return;
        }

        identity.username = username.clone();
        identity.save(&settings);

        ui_state.set(GuiState::MainMenu);
    }
}
