use bevy::prelude::*;

use super::{GuiState, InterfaceBundle, Interfaces};
use crate::{
    game_state::GameState,
    networking::{Identity, NetworkClient},
    singleplayer::LaunchSinglePlayer,
    ui::widgets::*,
};

pub struct MainMenuPlugin;
impl Plugin for MainMenuPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup).add_systems(
            Update,
            (press_singleplayer_button, press_join_button, goto_login)
                .run_if(in_state(GuiState::MainMenu)),
        );
    }
}

#[derive(Component)]
struct SinglePlayerButton;

#[derive(Component)]
struct ServerIp;

#[derive(Component)]
struct JoinButton;

fn setup(mut commands: Commands, mut interfaces: ResMut<Interfaces>) {
    let entity = commands
        .spawn(InterfaceBundle {
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
            background_color: Color::srgb_u8(33, 33, 33).into(),
            ..default()
        })
        .with_children(|parent| {
            parent
                .spawn_button(200.0, "Singleplayer")
                .insert(SinglePlayerButton);

            parent.spawn_textbox(200.0, "127.0.0.1").insert(ServerIp);
            parent.spawn_button(200.0, "Connect").insert(JoinButton);
        })
        .id();
    interfaces.insert(GuiState::MainMenu, entity);
}

// TODO: The button should lead to its own screen where you select game and save file
fn press_singleplayer_button(
    button_query: Query<&Interaction, (Changed<Interaction>, With<SinglePlayerButton>)>,
    mut launch_single_player: EventWriter<LaunchSinglePlayer>,
) {
    if let Ok(interaction) = button_query.get_single() {
        if *interaction == Interaction::Pressed {
            launch_single_player.send(LaunchSinglePlayer {});
        }
    }
}

fn press_join_button(
    mut net: ResMut<NetworkClient>,
    keys: Res<ButtonInput<KeyCode>>,
    server_ip: Query<&TextBox, With<ServerIp>>,
    play_button: Query<&Interaction, (Changed<Interaction>, With<JoinButton>)>,
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

        let addr = match ip.parse() {
            Ok(addr) => addr,
            Err(_) => return,
        };

        net.connect(addr);
        game_state.set(GameState::Connecting);
    }
}

fn goto_login(identity: Res<Identity>, mut gui_state: ResMut<NextState<GuiState>>) {
    if !identity.is_valid() {
        gui_state.set(GuiState::Login);
    }
}
