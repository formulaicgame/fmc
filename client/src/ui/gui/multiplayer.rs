use bevy::prelude::*;

use crate::{game_state::GameState, ui::widgets::*};

use super::{InterfaceBundle, Interfaces, UiState};

pub struct MultiPlayerPlugin;
impl Plugin for MultiPlayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup).add_systems(
            Update,
            press_play_button.run_if(in_state(UiState::MultiPlayer)),
        );
    }
}

#[derive(Component)]
struct ServerIp;

#[derive(Component)]
struct PlayButton;

fn setup(mut commands: Commands, mut interfaces: ResMut<Interfaces>) {
    let entity = commands
        .spawn(InterfaceBundle {
            background_color: Color::BLACK.into(),
            style: Style {
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: Val::Percent(2.0),
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                position_type: PositionType::Absolute,
                ..default()
            },
            ..default()
        })
        .with_children(|parent| {
            parent.spawn_textbox(41.5, "127.0.0.1").insert(ServerIp);
            parent.spawn_button(200.0, "PLAY").insert(PlayButton);
        })
        .id();
    interfaces.insert(UiState::MultiPlayer, entity);
}

fn press_play_button(
    mut net: ResMut<fmc_networking::NetworkClient>,
    keys: Res<ButtonInput<KeyCode>>,
    server_ip: Query<&TextBox, With<ServerIp>>,
    play_button: Query<&Interaction, (Changed<Interaction>, With<PlayButton>)>,
    mut game_state: ResMut<NextState<GameState>>,
) {
    if play_button
        .get_single()
        .is_ok_and(|interaction| *interaction == Interaction::Pressed)
        || keys.just_pressed(KeyCode::Enter)
    {
        let mut ip = server_ip.single().text.to_owned();

        if !ip.contains(":") {
            ip.push_str(":42069");
        }

        net.connect(ip.clone());
        game_state.set(GameState::Connecting);
    }
}
