use bevy::prelude::*;
use fmc_protocol::messages;

use super::{BACKGROUND, GuiState, Interface, Interfaces};
use crate::{
    assets::AssetState,
    game_state::GameState,
    networking::{NetworkClient, NetworkEvent},
    ui::client::{BASE_SIZE, widgets::*},
};

pub struct ConnectionStatusPlugin;
impl Plugin for ConnectionStatusPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<NetworkEvent>()
            .add_systems(Startup, setup)
            .add_systems(
                Update,
                (
                    press_cancel.run_if(in_state(GuiState::ConnectionStatus)),
                    downloading_assets_text.run_if(resource_added::<messages::ServerConfig>),
                    show_when_disconnected_for_reason.run_if(on_message::<NetworkEvent>),
                ),
            )
            .add_systems(OnEnter(GameState::Connecting), show_when_connecting)
            .add_systems(OnEnter(GameState::Playing), hide_on_game_start)
            .add_systems(OnEnter(AssetState::Loading), loading_assets_text);
    }
}

#[derive(Component)]
struct CancelButton;

#[derive(Component)]
struct StatusText;

fn setup(mut commands: Commands, mut interfaces: ResMut<Interfaces>) {
    let entity = commands
        .spawn((
            Interface,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                row_gap: Val::Px(20.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            ImageNode {
                image: BACKGROUND,
                image_mode: NodeImageMode::Stretch,
                ..default()
            },
        ))
        .with_children(|parent| {
            parent
                .spawn(Node {
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    width: Val::Percent(20.0),
                    ..default()
                })
                .with_children(|parent| {
                    parent
                        .spawn_text("Connecting to server...")
                        .insert(StatusText);
                });
            parent
                .spawn_button(
                    "Cancel",
                    ButtonStyle {
                        color: Color::srgb_u8(102, 97, 95),
                        width: Val::Percent(20.0),
                        height: BASE_SIZE * 20.0,
                        ..default()
                    },
                )
                .insert(CancelButton);
        })
        .id();
    interfaces.insert(GuiState::ConnectionStatus, entity);
}

fn press_cancel(
    net: Res<NetworkClient>,
    mut gui_state: ResMut<NextState<GuiState>>,
    button_query: Query<&Interaction, (Changed<Interaction>, With<CancelButton>)>,
) {
    if let Ok(interaction) = button_query.single() {
        if *interaction == Interaction::Pressed {
            net.disconnect("");
            gui_state.set(GuiState::MainMenu);
        }
    }
}

// TODO: Needs to display progress, but there's no visibility into it at the moment it's a [Local]
// over in 'src/networking.rs'.
fn downloading_assets_text(mut status_text: Query<&mut Text, With<StatusText>>) {
    let mut text = status_text.single_mut().unwrap();
    *text = Text::new("Downloading assets...");
}

fn loading_assets_text(mut status_text: Query<&mut Text, With<StatusText>>) {
    let mut text = status_text.single_mut().unwrap();
    *text = Text::new("Loading assets...");
}

fn show_when_disconnected_for_reason(
    mut status_text: Query<&mut Text, With<StatusText>>,
    mut next_gui_state: ResMut<NextState<GuiState>>,
    mut disconnect_events: MessageReader<NetworkEvent>,
) {
    for event in disconnect_events.read() {
        let NetworkEvent::Disconnected(reason) = event else {
            continue;
        };
        if reason.is_empty() {
            continue;
        }

        let mut text = status_text.single_mut().unwrap();
        *text = Text::new(reason);

        next_gui_state.set(GuiState::ConnectionStatus);
    }
}

fn show_when_connecting(
    mut status_text: Query<&mut Text, With<StatusText>>,
    mut gui_state: ResMut<NextState<GuiState>>,
) {
    *status_text.single_mut().unwrap() = Text::new("Connecting to server...");
    gui_state.set(GuiState::ConnectionStatus);
}

fn hide_on_game_start(mut gui_state: ResMut<NextState<GuiState>>) {
    gui_state.set(GuiState::None);
}
