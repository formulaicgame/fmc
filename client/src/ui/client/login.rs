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
                    width: Val::Px(200.0),
                    height: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    justify_content: JustifyContent::Center,
                    //align_items: AlignItems::Center,
                    ..default()
                })
                .with_children(|parent| {
                    parent.spawn_text("Enter username:");
                    parent.spawn_textbox("");
                    parent
                        .spawn_button("Play", Srgba::GREEN)
                        .insert(LoginButton);
                });
        })
        .id();
    interfaces.insert(GuiState::Login, entity);
}

fn press_play(
    mut ui_state: ResMut<NextState<GuiState>>,
    settings: Res<Settings>,
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
        identity.save(&settings);

        ui_state.set(GuiState::MainMenu);
    }
}
