use bevy::{
    prelude::*,
    tasks::{futures_lite::future, AsyncComputeTaskPool, Task},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use crate::{settings::Settings, singleplayer::SinglePlayerServer, ui::text_input::TextBox};

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
    settings: serde_json::Map<String, serde_json::Value>,
    // This task extracts the assets from the server executable that define the layout of
    // the configuration interfaces.
    ui_task: Option<Task<std::io::Result<std::process::ExitStatus>>>,
}

impl ConfiguredWorld {
    pub fn edit(&mut self, path: impl AsRef<Path>) {
        let path = path.as_ref();

        self.path = Some(path.to_path_buf());

        let Ok(connection) = rusqlite::Connection::open(path) else {
            warn!("Failed to open world database for editing");
            return;
        };

        let Ok(settings_json) = connection.query_row(
            "SELECT data FROM storage WHERE name='settings'",
            [],
            |row| row.get::<usize, String>(0),
        ) else {
            warn!("Failed to read settings from world database");
            return;
        };

        self.settings = match serde_json::from_str(&settings_json) {
            Ok(s) => s,
            Err(e) => {
                warn!("Could not deserialize settings from world database: {e}");
                return;
            }
        }
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
                                                .file_stem()
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
                    let Some(setting) = setting.as_str() else {
                        continue;
                    };

                    let Some(position) =
                        entries.iter().position(|e| e.eq_ignore_ascii_case(setting))
                    else {
                        continue;
                    };

                    *selected = position;
                }
                SettingsWidget::Switch { default_on, .. } => {
                    let Some(on) = setting.as_bool() else {
                        continue;
                    };
                    *default_on = on;
                }
                SettingsWidget::TextBox { text, .. } => {
                    let Some(setting) = setting.as_str() else {
                        continue;
                    };
                    *text = Some(setting.to_owned());
                }
                SettingsWidget::Slider { value, .. } => {
                    let Some(number) = setting.as_f64() else {
                        continue;
                    };
                    *value = number as f32;
                }
                SettingsWidget::Dropdown {
                    selected, entries, ..
                } => {
                    let Some(setting) = setting.as_str() else {
                        continue;
                    };

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
        configured_world.settings.insert(
            setting.name.clone(),
            serde_json::Value::from(text_box.text.as_str()),
        );
    }

    for (selection, setting) in button_selection_query.iter() {
        let value = selection.selected().to_lowercase().replace(" ", "_");
        configured_world
            .settings
            .insert(setting.name.clone(), serde_json::Value::from(value));
    }

    for (switch, setting) in switch_query.iter() {
        configured_world
            .settings
            .insert(setting.name.clone(), serde_json::Value::from(switch.on()));
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
    mut singleplayer_server: ResMut<SinglePlayerServer>,
    mut gui_state: ResMut<NextState<GuiState>>,
    world_name_input: Query<&TextBox, With<WorldName>>,
    button_query: Query<(&MainButtons, &Interaction), Changed<Interaction>>,
) {
    for (button, interaction) in button_query.iter() {
        if *interaction != Interaction::Pressed {
            continue;
        }

        match button {
            MainButtons::CreateWorld => {
                let mut world_name = world_name_input.single().unwrap().text.clone();
                if world_name.is_empty() {
                    world_name += "World";
                }

                let mut path = settings.data_dir().join("worlds");
                path.push(&world_name);
                path.set_extension("sqlite");

                let mut counter = 1;
                while path.exists() {
                    path.pop();
                    let new_name = world_name.clone() + " " + &counter.to_string();
                    path.push(new_name);
                    path.set_extension("sqlite");
                    counter += 1;
                }

                let connection = match rusqlite::Connection::open(&path) {
                    Ok(c) => c,
                    Err(e) => {
                        error!("Failed to create new world file: {e}");
                        gui_state.set(GuiState::MainMenu);
                        continue;
                    }
                };

                connection
                    .execute(
                        "create table if not exists storage (
                        name TEXT PRIMARY KEY,
                        data TEXT NOT NULL
                    )",
                        [],
                    )
                    .unwrap();

                connection
                    .execute(
                        "INSERT OR REPLACE INTO storage (name, data) VALUES (?,?)",
                        rusqlite::params![
                            "settings",
                            serde_json::to_string(&configured_world.settings).unwrap()
                        ],
                    )
                    .unwrap();
                singleplayer_server.start(path);
            }
            MainButtons::DeleteWorld => {
                if let Some(path) = &configured_world.path {
                    if let Err(e) = std::fs::remove_file(path) {
                        warn!("Encountered error when deleting a world: {e}");
                    };
                    gui_state.set(GuiState::MainMenu);
                }
            }
            MainButtons::Back => {
                if configured_world.is_editing() {
                    match rusqlite::Connection::open(&configured_world.path.as_ref().unwrap()) {
                        Ok(connection) => {
                            connection
                                .execute(
                                    "create table if not exists storage (
                                name TEXT PRIMARY KEY,
                                data TEXT NOT NULL
                            )",
                                    [],
                                )
                                .unwrap();

                            connection
                                .execute(
                                    "INSERT OR REPLACE INTO storage (name, data) VALUES (?,?)",
                                    rusqlite::params![
                                        "settings",
                                        serde_json::to_string(&configured_world.settings).unwrap()
                                    ],
                                )
                                .unwrap();
                        }
                        Err(e) => {
                            error!("Failed to open world file: {e}");
                        }
                    }

                    let world_name_input = &world_name_input.single().unwrap().text;
                    if !world_name_input.is_empty() {
                        let mut new_path = settings.data_dir().join("worlds");
                        new_path.push(&world_name_input);
                        new_path.set_extension("sqlite");

                        // TODO: Make the textbox red or display a notification if the world name
                        // is already taken.
                        if !new_path.exists() {
                            if let Err(e) =
                                std::fs::rename(configured_world.path.as_ref().unwrap(), new_path)
                            {
                                // TODO: Notification
                                error!("Couldn't rename world: {e}");
                            };
                        }
                    }
                }
                gui_state.set(GuiState::MainMenu);
            }
        }
    }
}
