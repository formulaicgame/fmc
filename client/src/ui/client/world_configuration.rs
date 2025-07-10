use bevy::{
    prelude::*,
    tasks::{futures_lite::future, AsyncComputeTaskPool, Task},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use crate::{settings::Settings, singleplayer::LaunchSinglePlayer, ui::text_input::TextBox};

use super::{
    widgets::{colors, ButtonSelection, ButtonStyle, SettingsWidget, Switch, Widgets},
    GuiState, Interface, Interfaces, BACKGROUND, BASE_SIZE,
};

pub struct WorldConfigurationPlugin;
impl Plugin for WorldConfigurationPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ConfiguredWorld::default())
            .add_systems(Startup, setup)
            .add_systems(OnEnter(GuiState::WorldConfiguration), build_interface)
            .add_systems(
                Update,
                (
                    handle_asset_extraction_task,
                    store_configuration_changes,
                    main_buttons,
                )
                    .run_if(in_state(GuiState::WorldConfiguration)),
            );
    }
}

fn setup(mut commands: Commands, mut interfaces: ResMut<Interfaces>) {
    let interface_root = commands
        .spawn((
            Interface,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                ..default()
            },
            ImageNode::new(BACKGROUND),
        ))
        .id();

    interfaces.insert(GuiState::WorldConfiguration, interface_root);
}

#[derive(Resource, Default, Debug)]
pub struct ConfiguredWorld {
    // Set to the path of the world that should be edited. If no path , the interface will let
    // the player create a new world.
    path: Option<PathBuf>,
    settings: HashMap<String, String>,
    ui_task: Option<Task<std::io::Result<std::process::ExitStatus>>>,
}

impl ConfiguredWorld {
    pub fn edit(&mut self, path: impl AsRef<Path>) {
        let path = path.as_ref();

        self.settings.clear();
        if let Ok(settings) = std::fs::read_to_string(path.join("settings.ini")) {
            for line in settings.lines() {
                if line.starts_with("#") {
                    continue;
                }

                let Some((left, right)) = line.split_once("=") else {
                    continue;
                };

                self.settings
                    .insert(left.trim().to_owned(), right.trim().to_owned());
            }
        }

        self.path = Some(path.to_path_buf());
    }

    fn is_editing(&self) -> bool {
        self.path.is_some()
    }

    pub fn new_world(&mut self) {
        self.path = None;
        self.settings.clear();
    }

    fn read_layout(&self) -> Option<WorldConfigurationUiLayout> {
        let path = if let Some(_path) = &self.path {
            Path::new("fmc_server/assets/client/interfaces/configuration/edit_world.json")
        } else {
            Path::new("fmc_server/assets/client/interfaces/configuration/create_world.json")
        };

        let file = match std::fs::File::open(&path) {
            Ok(f) => f,
            Err(e) => {
                error!("Failed to open {}: {e}", path.display());
                return None;
            }
        };

        match serde_json::from_reader(file) {
            Ok(l) => Some(l),
            Err(e) => {
                error!("Failed to read world configuration ui layout: {}", e);
                None
            }
        }
    }

    fn build_interface(&self, parent: &mut ChildSpawnerCommands, asset_server: &AssetServer) {
        let Some(mut layout) = self.read_layout() else {
            return;
        };

        if self.is_editing() {
            layout.apply_settings(self);
        }

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
                parent.spawn_tab("World", size, false).insert(Tabs::World);
                parent
                    .spawn_tab("Advanced", size, false)
                    .insert(Tabs::Advanced);
                parent.spawn_tab("Mods", size, false).insert(Tabs::Mods);
            });

        for (tab, widgets) in [
            (Tabs::Settings, layout.settings),
            (Tabs::World, layout.world),
            (Tabs::Advanced, layout.advanced),
        ] {
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
                .with_children(|parent| {
                    // The world name is the only field that's hard coded, always appears at the top.
                    if tab == Tabs::Settings {
                        parent
                            .spawn(Node {
                                width: Val::Percent(100.0),
                                height: BASE_SIZE * 29.0,
                                row_gap: BASE_SIZE * 4.0,
                                flex_direction: FlexDirection::Column,
                                ..default()
                            })
                            .with_children(|parent| {
                                parent.spawn_text("World name");
                                parent
                                    .spawn_textbox(TextBox {
                                        placeholder_text: "World".to_owned(),
                                        text: if self.is_editing() {
                                            // Name of the folder
                                            self.path
                                                .as_ref()
                                                .unwrap()
                                                .file_name()
                                                .unwrap_or_default()
                                                .to_str()
                                                .unwrap_or_default()
                                                .to_owned()
                                        } else {
                                            String::new()
                                        },
                                        ..default()
                                    })
                                    .insert(WorldName);
                            });
                    }

                    for setting_widget in widgets {
                        let setting = Setting {
                            name: setting_widget.normalized_name(),
                        };
                        setting_widget
                            .spawn(parent, asset_server, "fmc_server/assets/client")
                            .insert(setting);
                    }
                });
        }

        parent.spawn_footer();

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
                                "Back",
                                ButtonStyle {
                                    color: Color::srgb_u8(102, 97, 95),
                                    height: BASE_SIZE * 17.0,
                                    width: Val::Percent(78.0),
                                    ..default()
                                },
                            )
                            .insert(MainButtons::Back);
                    });
            });

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
                        if self.is_editing() {
                            parent
                                .spawn_button(
                                    "Delete",
                                    ButtonStyle {
                                        color: colors::BUTTON_RED,
                                        height: BASE_SIZE * 17.0,
                                        width: Val::Percent(78.0),
                                        ..default()
                                    },
                                )
                                .insert(MainButtons::DeleteWorld);
                        } else {
                            parent
                                .spawn_button(
                                    "Create World",
                                    ButtonStyle {
                                        color: colors::BUTTON_GREEN,
                                        height: BASE_SIZE * 17.0,
                                        width: Val::Percent(78.0),
                                        ..default()
                                    },
                                )
                                .insert(MainButtons::CreateWorld);
                        }
                    });
            });
    }
}

#[derive(Deserialize, Serialize, Default)]
struct WorldConfigurationUiLayout {
    // This is used in the pause interface too
    settings: Vec<SettingsWidget>,
    world: Vec<SettingsWidget>,
    advanced: Vec<SettingsWidget>,
    mods: Vec<SettingsWidget>,
}

impl WorldConfigurationUiLayout {
    fn apply_settings(&mut self, configured_world: &ConfiguredWorld) {
        for widget in self
            .settings
            .iter_mut()
            .chain(self.world.iter_mut())
            .chain(self.advanced.iter_mut())
        {
            let Some(setting) = configured_world.settings.get(&widget.normalized_name()) else {
                continue;
            };

            match widget {
                SettingsWidget::ButtonSelection {
                    selected, entries, ..
                } => {
                    let Some(position) =
                        entries.iter().position(|e| e.eq_ignore_ascii_case(setting))
                    else {
                        continue;
                    };
                    *selected = position;
                }
                SettingsWidget::Switch { default_on, .. } => {
                    let Ok(on) = setting.parse() else {
                        continue;
                    };
                    *default_on = on;
                }
                SettingsWidget::TextBox { text, .. } => {
                    *text = Some(setting.to_owned());
                }
                SettingsWidget::Slider { value, .. } => {
                    let Ok(number) = setting.parse() else {
                        continue;
                    };
                    *value = number;
                }
                SettingsWidget::Dropdown {
                    selected, entries, ..
                } => {
                    let Some(position) = entries.iter().position(|e| e == setting) else {
                        continue;
                    };

                    *selected = position;
                }
            }
        }
    }
}

#[derive(Component)]
struct Setting {
    name: String,
}

#[derive(Component)]
enum MainButtons {
    CreateWorld,
    DeleteWorld,
    Back,
}

#[derive(Component, PartialEq, Eq, Clone, Copy, Hash, Deserialize)]
enum Tabs {
    World,
    Settings,
    Advanced,
    Mods,
}

#[derive(Component)]
struct WorldName;

fn store_configuration_changes(
    mut configured_world: ResMut<ConfiguredWorld>,
    text_box_query: Query<(&TextBox, &Setting), Changed<TextBox>>,
    button_selection_query: Query<(&ButtonSelection, &Setting), Changed<ButtonSelection>>,
    switch_query: Query<(&Switch, &Setting), Changed<Switch>>,
) {
    for (text_box, setting) in text_box_query.iter() {
        if !text_box.text.is_empty() {
            configured_world
                .settings
                .insert(setting.name.clone(), text_box.text.clone());
        } else {
            configured_world.settings.remove(&setting.name);
        }
    }

    for (selection, setting) in button_selection_query.iter() {
        let value = selection.selected().to_lowercase().replace(" ", "_");
        configured_world
            .settings
            .insert(setting.name.clone(), value);
    }

    for (switch, setting) in switch_query.iter() {
        configured_world
            .settings
            .insert(setting.name.clone(), switch.on().to_string());
    }
}

fn build_interface(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut configured_world: ResMut<ConfiguredWorld>,
    interfaces: Res<Interfaces>,
) {
    if Path::new("fmc_server/assets/").exists() {
        let Some(layout) = configured_world.read_layout() else {
            return;
        };

        commands
            .entity(*interfaces.get(&GuiState::WorldConfiguration).unwrap())
            .despawn_related::<Children>()
            .with_children(|parent| {
                configured_world.build_interface(parent, &asset_server);
            });
    } else {
        // TODO: This should be done where the game is downloaded and placed along all the other
        // server assets so that they do not need to be downloaded from the server on connection.
        let server_path =
            Path::new("fmc_server/server").with_extension(std::env::consts::EXE_SUFFIX);
        if !server_path.exists() {
            // The main menu should block entering the interface when there is no server, but we check
            // anyway.
            return;
        }

        let task_pool = AsyncComputeTaskPool::get();
        configured_world.ui_task = Some(task_pool.spawn(async move {
            std::process::Command::new(&std::fs::canonicalize(server_path).unwrap())
                .current_dir("fmc_server")
                // The server listens for this in order to organize its files differently when running
                // as a cargo project. We don't want that when running it through the client.
                .env_remove("CARGO")
                .arg("--extract-assets")
                .status()
        }));
    }
}

fn handle_asset_extraction_task(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut configured_world: ResMut<ConfiguredWorld>,
    interfaces: Res<Interfaces>,
) {
    let Some(Some(result)) = &configured_world
        .ui_task
        .as_mut()
        .map(|task| future::block_on(future::poll_once(task)))
    else {
        return;
    };

    configured_world.ui_task = None;

    commands
        .entity(*interfaces.get(&GuiState::WorldConfiguration).unwrap())
        .despawn_related::<Children>()
        .with_children(|parent| {
            configured_world.build_interface(parent, &asset_server);
        });
}

fn main_buttons(
    settings: Res<Settings>,
    configured_world: Res<ConfiguredWorld>,
    mut gui_state: ResMut<NextState<GuiState>>,
    world_name_input: Query<&TextBox, With<WorldName>>,
    button_query: Query<(&MainButtons, &Interaction), Changed<Interaction>>,
    mut launch_single_player: EventWriter<LaunchSinglePlayer>,
) {
    for (button, interaction) in button_query.iter() {
        if *interaction != Interaction::Pressed {
            continue;
        }

        match button {
            MainButtons::CreateWorld => {
                let mut world_input = world_name_input.single().unwrap().text.clone();
                if world_input.is_empty() {
                    world_input += "World";
                }

                let mut path = settings.data_dir().join("worlds");
                path.push(&world_input);

                let mut counter = 1;
                while path.exists() {
                    path.pop();
                    let new_name = world_input.clone() + " " + &counter.to_string();
                    path.push(new_name);
                    counter += 1;
                }
                std::fs::create_dir(&path).ok();

                let mut settings = String::new();
                for (key, value) in &configured_world.settings {
                    settings.push_str(&format!("{} = {}\n", key, value));
                }

                let settings_path = path.join("settings.ini");
                std::fs::write(settings_path, settings).unwrap();

                launch_single_player.write(LaunchSinglePlayer { path });
            }
            MainButtons::DeleteWorld => {
                if let Some(path) = &configured_world.path {
                    if let Err(e) = std::fs::remove_dir_all(path) {
                        warn!("Encountered error when deleting a world: {e}");
                    };
                    gui_state.set(GuiState::MainMenu);
                }
            }
            MainButtons::Back => {
                if configured_world.is_editing() {
                    let mut settings = String::new();
                    for (key, value) in &configured_world.settings {
                        settings.push_str(&format!("{} = {}\n", key, value));
                    }

                    let path = configured_world.path.as_ref().unwrap().join("settings.ini");
                    std::fs::write(path, settings).unwrap();
                }
                gui_state.set(GuiState::MainMenu);
            }
        }
    }
}
