use bevy::{
    prelude::*,
    ui::{widget::UiImageSize, ContentSize},
};

use super::{GuiState, InterfaceBundle, Interfaces};
use crate::{networking::Identity, singleplayer::LaunchSinglePlayer, ui::widgets::*};

pub struct MainMenuPlugin;
impl Plugin for MainMenuPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup).add_systems(
            Update,
            (press_singleplayer, press_multiplayer, goto_login)
                .run_if(in_state(GuiState::MainMenu)),
        );
    }
}

#[derive(Component)]
struct SinglePlayerButton;
#[derive(Component)]
struct MultiPlayerButton;

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
                row_gap: Val::Px(4.0),
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
                .spawn_button(200.0, "Singleplayer")
                .insert(SinglePlayerButton);
            parent
                .spawn_button(200.0, "Multiplayer")
                .insert(MultiPlayerButton);
        })
        .id();
    interfaces.insert(GuiState::MainMenu, entity);
}

// TODO: The button should lead to its own screen where you select game and save file
fn press_singleplayer(
    button_query: Query<&Interaction, (Changed<Interaction>, With<SinglePlayerButton>)>,
    mut launch_single_player: EventWriter<LaunchSinglePlayer>,
) {
    if let Ok(interaction) = button_query.get_single() {
        if *interaction == Interaction::Pressed {
            launch_single_player.send(LaunchSinglePlayer {});
        }
    }
}

fn press_multiplayer(
    mut ui_state: ResMut<NextState<GuiState>>,
    button_query: Query<&Interaction, (Changed<Interaction>, With<MultiPlayerButton>)>,
) {
    if let Ok(interaction) = button_query.get_single() {
        if *interaction == Interaction::Pressed {
            ui_state.set(GuiState::MultiPlayer);
        }
    }
}

fn goto_login(identity: Res<Identity>, mut gui_state: ResMut<NextState<GuiState>>) {
    if !identity.is_valid() {
        gui_state.set(GuiState::Login);
    }
}
