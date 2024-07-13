use bevy::{
    prelude::*,
    ui::{widget::UiImageSize, ContentSize},
};
use fmc_networking::NetworkClient;

use super::{InterfaceBundle, Interfaces, UiState};
use crate::{assets::AssetState, game_state::GameState, ui::widgets::*};

pub struct ConnectingPlugin;
impl Plugin for ConnectingPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup)
            .add_systems(Update, press_cancel.run_if(in_state(UiState::Connecting)))
            .add_systems(OnEnter(AssetState::Downloading), downloading_assets_text)
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
            background_color: Color::ANTIQUE_WHITE.into(),
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
    interfaces.insert(UiState::Connecting, entity);
}

fn press_cancel(
    net: Res<NetworkClient>,
    mut game_state: ResMut<NextState<GameState>>,
    button_query: Query<&Interaction, (Changed<Interaction>, With<CancelButton>)>,
) {
    if let Ok(interaction) = button_query.get_single() {
        if *interaction == Interaction::Pressed {
            net.disconnect("");
            game_state.set(GameState::Launcher);
        }
    }
}

fn downloading_assets_text(mut status_text: Query<&mut Text, With<StatusText>>) {
    let mut text = status_text.single_mut();
    text.sections[0].value = "Downloading assets...".to_owned();
}

fn loading_assets_text(mut status_text: Query<&mut Text, With<StatusText>>) {
    let mut text = status_text.single_mut();
    text.sections[0].value = "Downloading assets...".to_owned();
}
