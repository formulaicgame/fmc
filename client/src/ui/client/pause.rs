use bevy::{
    audio::{PlaybackSettings, Volume},
    color::palettes::css::DARK_GRAY,
    prelude::*,
    window::WindowFocused,
};
use fmc_protocol::messages;
use serde::Deserialize;

use super::{GuiState, Interface, Interfaces, BASE_SIZE};
use crate::{
    game_state::GameState,
    networking::NetworkClient,
    player::Head,
    settings::Settings,
    ui::{
        client::widgets::{colors, ButtonSelection, ButtonStyle, SettingsWidget, Slider, Widgets},
        text_input::TextBox,
        Scale,
    },
};

// TODO: Doesn't actually pause the game! Send PauseRequest to server, if all players have sent a
// request change state to GameState::Paused, GameState doesn't exist.
pub struct PausePlugin;
impl Plugin for PausePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                setup.run_if(resource_exists_and_changed::<messages::ServerConfig>),
                (
                    navigation_buttons,
                    switch_tab,
                    video_settings,
                    audio_settings,
                    control_settings,
                )
                    // This is the only client interface that is shown during gameplay and that
                    // will interact with the server directly. It would be nice to only listen for
                    // the PauseMenu state, but the gui is not designed for this. On disconnect the
                    // GuiState reacts to the change in GameState, which is necessarily always
                    // 1-frame delayed, allowing access to the network client for the duration. The
                    // network client is considered invalid to interact with when not in
                    // GameState::Playing, and will panic if attempted (which we do here).
                    .run_if(in_state(GuiState::PauseMenu).and(in_state(GameState::Playing))),
                (pause_when_unfocused, escape_key, server_settings)
                    .run_if(in_state(GameState::Playing)),
            ),
        );
    }
}

#[derive(Component)]
enum NavigationButtons {
    Resume,
    Quit,
}

#[derive(Component, PartialEq, Eq, Clone, Copy, Hash)]
enum Tabs {
    Settings,
    Video,
    Audio,
    Controls,
}

impl Tabs {
    fn settings_layout() -> Vec<SettingsWidget> {
        #[derive(Deserialize)]
        struct ServerSettings {
            settings: Vec<SettingsWidget>,
        }

        let path = "server_assets/active/interfaces/configuration/edit_world.json";
        let file = match std::fs::File::open(path) {
            Ok(f) => f,
            Err(e) => {
                error!("Failed to open {path}: {e}");
                return Vec::new();
            }
        };

        match serde_json::from_reader::<_, ServerSettings>(file) {
            Ok(l) => l.settings,
            Err(e) => {
                error!("Failed to read world configuration ui layout: {}", e);
                Vec::new()
            }
        }
    }

    fn video_layout(
        settings: &Settings,
        server_config: &messages::ServerConfig,
    ) -> Vec<(VideoSettings, SettingsWidget)> {
        vec![
            (
                VideoSettings::Resolution,
                SettingsWidget::Dropdown {
                    name: "Resolution".to_owned(),
                    description: None,
                    entries: vec!["3840x2160".to_owned(), "1920x1080".to_owned()],
                    selected: 0,
                },
            ),
            (
                VideoSettings::GuiScale,
                SettingsWidget::Slider {
                    name: "Gui scale".to_owned(),
                    description: None,
                    min: 0.1,
                    max: 2.0,
                    value: settings.ui_scale,
                    decimals: 1,
                    unit: "x".to_owned(),
                },
            ),
            (
                VideoSettings::RenderDistance,
                SettingsWidget::Slider {
                    name: "Render distance".to_owned(),
                    description: None,
                    min: 1.0,
                    max: server_config.render_distance as f32,
                    value: settings.render_distance.min(server_config.render_distance) as f32,
                    decimals: 0,
                    unit: " chunks".to_owned(),
                },
            ),
            (
                VideoSettings::FieldOfView,
                SettingsWidget::Slider {
                    name: "Field of view".to_owned(),
                    description: None,
                    min: 50.0,
                    max: 130.0,
                    value: settings.fov,
                    decimals: 0,
                    unit: "°".to_owned(),
                },
            ),
            (
                VideoSettings::Silly,
                SettingsWidget::Switch {
                    name: "Silly switch".to_owned(),
                    description: None,
                    default_on: false,
                },
            ),
        ]
    }

    fn audio_layout(settings: &Settings) -> Vec<(AudioSettings, SettingsWidget)> {
        vec![(
            AudioSettings::Volume,
            SettingsWidget::Slider {
                name: "Volume".to_owned(),
                description: None,
                min: 0.0,
                max: 100.0,
                value: settings.volume * 100.0,
                decimals: 0,
                unit: "%".to_owned(),
            },
        )]
    }

    fn controls_layout(settings: &Settings) -> Vec<(ControlSettings, SettingsWidget)> {
        vec![(
            ControlSettings::Sensitivity,
            SettingsWidget::Slider {
                name: "Sensitivity".to_owned(),
                description: None,
                min: 0.01,
                max: 4.0,
                value: settings.sensitivity,
                decimals: 2,
                unit: "x".to_owned(),
            },
        )]
    }
}

#[derive(Component)]
enum VideoSettings {
    Resolution,
    GuiScale,
    RenderDistance,
    FieldOfView,
    Silly,
}

#[derive(Component)]
enum AudioSettings {
    Volume,
}

#[derive(Component)]
enum ControlSettings {
    Sensitivity,
}

fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    settings: Res<Settings>,
    server_config: Res<messages::ServerConfig>,
    mut interfaces: ResMut<Interfaces>,
) {
    if let Some(entity) = interfaces.remove(&GuiState::PauseMenu) {
        commands.entity(entity).despawn();
    }

    let entity = commands
        .spawn((
            Interface,
            Node {
                display: Display::None,
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                ..default()
            },
        ))
        .with_children(|parent| {
            parent.spawn_header();

            // Tabs
            parent
                .spawn(Node {
                    width: Val::Percent(100.0),
                    height: BASE_SIZE * 27.0,
                    column_gap: BASE_SIZE * 7.0,
                    justify_content: JustifyContent::Center,
                    ..default()
                })
                .with_children(|parent| {
                    let size = BASE_SIZE * 105.0;
                    parent
                        .spawn_tab("Settings", size, true)
                        .insert(Tabs::Settings);
                    parent.spawn_tab("Video", size, false).insert(Tabs::Video);
                    parent.spawn_tab("Audio", size, false).insert(Tabs::Audio);
                    parent
                        .spawn_tab("Controls", size, false)
                        .insert(Tabs::Controls);
                });

            for tab in [Tabs::Settings, Tabs::Video, Tabs::Audio, Tabs::Controls] {
                parent
                    .spawn((
                        Node {
                            display: if tab == Tabs::Settings {
                                Display::Flex
                            } else {
                                Display::None
                            },
                            margin: UiRect::top(BASE_SIZE * 14.0),
                            width: Val::Percent(56.0),
                            // height: BASE_SIZE * 243.0,
                            flex_grow: 1.0,
                            row_gap: BASE_SIZE * 9.0,
                            overflow: Overflow::clip(),
                            align_items: AlignItems::Center,
                            flex_direction: FlexDirection::Column,
                            ..default()
                        },
                        tab,
                    ))
                    .with_children(|parent| match tab {
                        Tabs::Settings => {
                            for widget in Tabs::settings_layout() {
                                let setting = ServerSetting {
                                    name: widget.normalized_name(),
                                };
                                widget
                                    .spawn(parent, &asset_server, "server_assets/active")
                                    .insert(setting);
                            }
                        }
                        Tabs::Video => {
                            for (marker, widget) in Tabs::video_layout(&settings, &server_config) {
                                widget
                                    .spawn(parent, &asset_server, "server_assets/active")
                                    .insert(marker);
                            }
                        }
                        Tabs::Audio => {
                            for (marker, widget) in Tabs::audio_layout(&settings) {
                                widget
                                    .spawn(parent, &asset_server, "server_assets/active")
                                    .insert(marker);
                            }
                        }
                        Tabs::Controls => {
                            for (marker, widget) in Tabs::controls_layout(&settings) {
                                widget
                                    .spawn(parent, &asset_server, "server_assets/active")
                                    .insert(marker);
                            }
                        }
                    });
            }

            parent.spawn_footer();

            // Create world button / delete world button
            parent
                .spawn(Node {
                    position_type: PositionType::Absolute,
                    flex_direction: FlexDirection::Column,
                    // Elevates it over the second line of the footer
                    bottom: BASE_SIZE * 3.0,
                    left: Val::Px(0.0),
                    height: BASE_SIZE * 28.0,
                    width: Val::Percent(15.0),
                    ..default()
                })
                .with_children(|parent| {
                    parent.spawn((
                        Node {
                            height: BASE_SIZE * 3.0,
                            width: Val::Percent(100.0),
                            ..default()
                        },
                        BackgroundColor::from(Srgba::rgba_u8(109, 99, 89, 255)),
                    ));
                    parent
                        .spawn((
                            Node {
                                height: BASE_SIZE * 25.0,
                                width: Val::Percent(100.0),
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                ..default()
                            },
                            BackgroundColor::from(Color::BLACK),
                        ))
                        .with_children(|parent| {
                            parent
                                .spawn_button(
                                    "Resume (Esc)",
                                    ButtonStyle {
                                        color: colors::BUTTON_GREEN,
                                        height: BASE_SIZE * 17.0,
                                        width: Val::Percent(78.0),
                                        ..default()
                                    },
                                )
                                .insert(NavigationButtons::Resume);
                        });
                });

            // Back button
            parent
                .spawn(Node {
                    position_type: PositionType::Absolute,
                    flex_direction: FlexDirection::Column,
                    // Elevates it over the second line of the footer
                    bottom: BASE_SIZE * 3.0,
                    right: Val::Px(0.0),
                    height: BASE_SIZE * 28.0,
                    width: Val::Percent(12.5),
                    ..default()
                })
                .with_children(|parent| {
                    parent.spawn((
                        Node {
                            height: BASE_SIZE * 3.0,
                            width: Val::Percent(100.0),
                            ..default()
                        },
                        BackgroundColor::from(Srgba::rgba_u8(109, 99, 89, 255)),
                    ));
                    parent
                        .spawn((
                            Node {
                                height: BASE_SIZE * 25.0,
                                width: Val::Percent(100.0),
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                ..default()
                            },
                            BackgroundColor::from(Color::BLACK),
                        ))
                        .with_children(|parent| {
                            parent
                                .spawn_button(
                                    "Quit",
                                    ButtonStyle {
                                        color: Color::srgb_u8(102, 97, 95),
                                        height: BASE_SIZE * 17.0,
                                        width: Val::Percent(78.0),
                                        ..default()
                                    },
                                )
                                .insert(NavigationButtons::Quit);
                        });
                });
        })
        .id();
    interfaces.insert(GuiState::PauseMenu, entity);
}

#[derive(Component)]
struct ServerSetting {
    name: String,
}

fn server_settings(
    net: Res<NetworkClient>,
    mut setting_updates: EventReader<messages::GuiSetting>,
    mut sliders: Query<(Mut<Slider>, &ServerSetting)>,
    mut text_boxes: Query<(Mut<TextBox>, &ServerSetting)>,
    mut button_selections: Query<(Mut<ButtonSelection>, &ServerSetting)>,
) {
    for (mut selection, setting) in button_selections.iter_mut() {
        if !selection.is_changed() || selection.is_added() {
            continue;
        }

        net.send_message(messages::GuiSetting::ButtonSelection {
            name: setting.name.clone(),
            selected: selection.index(),
        });
    }

    for setting_update in setting_updates.read() {
        match setting_update {
            messages::GuiSetting::TextBox { name, value } => {
                for (mut text_box, setting) in text_boxes.iter_mut() {
                    if name == &setting.name {
                        text_box.text = value.clone();
                        break;
                    }
                }
            }
            messages::GuiSetting::ButtonSelection { name, selected } => {
                for (mut selection, setting) in button_selections.iter_mut() {
                    if name == &setting.name {
                        selection.set_selected(*selected);
                        break;
                    }
                }
            }
            messages::GuiSetting::Slider { name, value } => {
                for (mut slider, setting) in sliders.iter_mut() {
                    if name == &setting.name {
                        slider.set_value(*value);
                        break;
                    }
                }
            }
            _ => (),
        }
    }
}

fn video_settings(
    net: Res<NetworkClient>,
    mut settings: ResMut<Settings>,
    mut ui_scale: ResMut<Scale>,
    mut camera: Query<&mut Projection, With<Head>>,
    sliders: Query<(&Slider, &VideoSettings), Changed<Slider>>,
) {
    for (slider, video_settings) in sliders.iter() {
        match video_settings {
            VideoSettings::RenderDistance => {
                let new_distance = slider.value() as u32;
                if settings.render_distance != new_distance {
                    settings.render_distance = slider.value() as u32;
                    net.send_message(messages::RenderDistance {
                        chunks: settings.render_distance,
                    });
                }
            }
            VideoSettings::FieldOfView => {
                let mut projection = camera.single_mut().unwrap();
                match *projection {
                    Projection::Perspective(ref mut projection) => {
                        settings.fov = slider.value();
                        projection.fov = slider.value().to_radians();
                    }
                    _ => (),
                }
            }
            VideoSettings::GuiScale => {
                settings.ui_scale = slider.value();
                ui_scale.set_scale(slider.value());
            }
            _ => (),
        }
    }
}

fn audio_settings(
    mut settings: ResMut<Settings>,
    mut global_volume: ResMut<GlobalVolume>,
    mut sinks: Query<(&mut AudioSink, &PlaybackSettings)>,
    sliders: Query<(&Slider, &AudioSettings), Changed<Slider>>,
) {
    for (slider, audio_setting) in sliders.iter() {
        match audio_setting {
            AudioSettings::Volume => {
                let new_volume = slider.value() / 100.0;
                settings.volume = new_volume;
                global_volume.volume = Volume::Linear(new_volume);
                for (mut sink, playback_settings) in sinks.iter_mut() {
                    sink.set_volume(global_volume.volume * playback_settings.volume);
                }
            }
        }
    }
}

fn control_settings(
    mut settings: ResMut<Settings>,
    sliders: Query<(&Slider, &ControlSettings), Changed<Slider>>,
) {
    for (slider, control_setting) in sliders.iter() {
        match control_setting {
            ControlSettings::Sensitivity => {
                settings.sensitivity = slider.value();
            }
        }
    }
}

fn navigation_buttons(
    net: Res<NetworkClient>,
    mut gui_state: ResMut<NextState<GuiState>>,
    button_query: Query<(&Interaction, &NavigationButtons), Changed<Interaction>>,
) {
    for (interaction, button) in button_query.iter() {
        if *interaction != Interaction::Pressed {
            continue;
        }

        match button {
            NavigationButtons::Resume => {
                gui_state.set(GuiState::None);
            }
            NavigationButtons::Quit => {
                gui_state.set(GuiState::MainMenu);
                net.disconnect("");
            }
        }
    }
}

fn escape_key(
    gui_state: Res<State<GuiState>>,
    mut next_gui_state: ResMut<NextState<GuiState>>,
    input: Res<ButtonInput<KeyCode>>,
) {
    if input.just_pressed(KeyCode::Escape) {
        if *gui_state.get() == GuiState::PauseMenu {
            next_gui_state.set(GuiState::None);
        } else {
            next_gui_state.set(GuiState::PauseMenu);
        }
    }
}

// TODO: If the client was paused by being unfocused it should unpause when focused again.
fn pause_when_unfocused(
    mut gui_state: ResMut<NextState<GuiState>>,
    mut focus_events: EventReader<WindowFocused>,
) {
    for event in focus_events.read() {
        if !event.focused {
            gui_state.set(GuiState::PauseMenu);
        }
    }
}

fn switch_tab(
    mut tab_buttons: Query<(&Tabs, &Interaction, &mut Node, &mut BackgroundColor)>,
    mut tab_content: Query<(&mut Node, &mut Visibility, &Tabs), Without<Interaction>>,
) {
    let mut clicked_tab = None;
    for (tab, interaction, _, _) in tab_buttons.iter() {
        if *interaction == Interaction::Pressed {
            clicked_tab = Some(*tab);
            break;
        }
    }
    let Some(clicked_tab) = clicked_tab else {
        return;
    };

    for (tab, interaction, mut node, mut color) in tab_buttons.iter_mut() {
        if clicked_tab == *tab {
            node.height = Val::Px(24.0);
            node.margin = UiRect::top(BASE_SIZE * 3.0);
            *color = BackgroundColor::from(colors::TAB_ACTIVE);
        } else {
            node.height = Val::Px(24.0);
            node.margin = UiRect::default();
            *color = BackgroundColor::from(colors::TAB_INACTIVE);
        }
    }

    for (mut node, mut visibility, tab_content) in tab_content.iter_mut() {
        if clicked_tab == *tab_content {
            node.display = Display::Flex;
            *visibility = Visibility::Inherited;
        } else {
            // Setting display to None makes it not affect the layout.
            // Text boxes de-focus when their visibility is changed, so we have to set that
            // too so the search bars don't stay focused when switching tabs.
            node.display = Display::None;
            *visibility = Visibility::Hidden;
        }
    }
}
