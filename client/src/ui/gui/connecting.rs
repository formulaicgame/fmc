use bevy::{
    prelude::*,
    ui::{widget::UiImageSize, ContentSize},
};
use fmc_protocol::messages;

use super::{GuiState, InterfaceBundle, Interfaces};
use crate::{assets::AssetState, game_state::GameState, networking::NetworkClient, ui::widgets::*};

// TODO: I think this looks better as an event architecture. You have something you want to
// show in the connection ui -> you send an event with the string you want shown -> the ui is
// shown. No logic needed for when to enter during connection, and no logic needed to enter when
// disconnecting by some network error.
pub struct ConnectingPlugin;
impl Plugin for ConnectingPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup)
            .add_systems(
                Update,
                (
                    press_cancel.run_if(in_state(GuiState::Connecting)),
                    downloading_assets_text.run_if(resource_added::<messages::ServerConfig>),
                    (disconnect_text, show_when_disconnected_for_reason)
                        .run_if(on_event::<messages::Disconnect>()),
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

fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut interfaces: ResMut<Interfaces>,
) {
    let entity = commands
        .spawn(InterfaceBundle {
            style: Style {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                row_gap: Val::Px(20.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            ..default()
        })
        .insert((
            ContentSize::default(),
            UiImageSize::default(),
            UiImage::from(
                asset_server.load::<Image>("embedded://client/ui/gui/assets/background.png"),
            ),
            ImageScaleMode::Tiled {
                tile_x: true,
                tile_y: true,
                stretch_value: 2.0,
            },
        ))
        .with_children(|parent| {
            parent
                .spawn(NodeBundle {
                    style: Style {
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                        width: Val::Percent(100.0),
                        ..default()
                    },
                    ..default()
                })
                .with_children(|parent| {
                    parent
                        .spawn_text("Connecting to server...")
                        .insert(StatusText);
                });
            parent.spawn_button(200.0, "Cancel").insert(CancelButton);
        })
        .id();
    interfaces.insert(GuiState::Connecting, entity);
}

fn press_cancel(
    net: Res<NetworkClient>,
    mut game_state: ResMut<NextState<GuiState>>,
    button_query: Query<&Interaction, (Changed<Interaction>, With<CancelButton>)>,
) {
    if let Ok(interaction) = button_query.get_single() {
        if *interaction == Interaction::Pressed {
            net.disconnect("");
            game_state.set(GuiState::MainMenu);
        }
    }
}

// TODO: Needs to display progress, but there's no visibility into it at the moment it's a Local
// over in 'src/networking.rs'.
fn downloading_assets_text(mut status_text: Query<&mut Text, With<StatusText>>) {
    let mut text = status_text.single_mut();
    text.sections[0].value = "Downloading assets...".to_owned();
}

fn loading_assets_text(mut status_text: Query<&mut Text, With<StatusText>>) {
    let mut text = status_text.single_mut();
    text.sections[0].value = "Loading assets...".to_owned();
}

fn disconnect_text(
    mut status_text: Query<&mut Text, With<StatusText>>,
    mut disconnect_events: EventReader<messages::Disconnect>,
) {
    for disconnect_event in disconnect_events.read() {
        let mut text = status_text.single_mut();
        text.sections[0].value = disconnect_event.message.to_owned();
    }
}

fn show_when_disconnected_for_reason(
    gui_state: Res<State<GuiState>>,
    mut next_gui_state: ResMut<NextState<GuiState>>,
    mut disconnect_events: EventReader<messages::Disconnect>,
) {
    for event in disconnect_events.read() {
        if event.message.is_empty() || *gui_state.get() != GuiState::None {
            continue;
        }

        next_gui_state.set(GuiState::Connecting);
    }
}

fn show_when_connecting(mut gui_state: ResMut<NextState<GuiState>>) {
    gui_state.set(GuiState::Connecting);
}

fn hide_on_game_start(mut gui_state: ResMut<NextState<GuiState>>) {
    gui_state.set(GuiState::None);
}
