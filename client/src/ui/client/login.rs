use bevy::prelude::*;

use super::{GuiState, Interface, Interfaces};
use crate::{networking::Identity, ui::widgets::*};

pub struct LoginPlugin;
impl Plugin for LoginPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, interface_setup)
            .add_systems(Update, (press_play.run_if(in_state(GuiState::Login)),));
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
            BackgroundColor::from(Color::srgb_u8(33, 33, 33)),
        ))
        .with_children(|parent| {
            parent
                .spawn(Node {
                    width: Val::Percent(100.0),
                    height: Val::Px(12.0),
                    justify_content: JustifyContent::Center,
                    ..default()
                })
                .with_children(|parent| {
                    parent.spawn_text("Enter username:");
                });
            parent.spawn_textbox(200.0, "").insert(Username);
            parent.spawn_button(200.0, "Play").insert(LoginButton);
        })
        .id();
    interfaces.insert(GuiState::Login, entity);
}

fn press_play(
    mut ui_state: ResMut<NextState<GuiState>>,
    mut identity: ResMut<Identity>,
    username: Query<&TextBox, With<Username>>,
    button_query: Query<&Interaction, (Changed<Interaction>, With<LoginButton>)>,
) {
    if let Ok(interaction) = button_query.get_single() {
        if *interaction != Interaction::Pressed {
            return;
        }

        let username = &username.single().text;

        if username.is_empty() {
            return;
        }

        identity.username = username.clone();

        std::fs::write("./identity.txt", &identity.username).ok();

        ui_state.set(GuiState::MainMenu);
    }
}
