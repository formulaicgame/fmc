use std::{
    io::{ErrorKind, Read, Write},
    path::{Path, PathBuf},
};

use bevy::{
    asset::{load_internal_binary_asset, weak_handle, RenderAssetUsages},
    image::{CompressedImageFormats, ImageSampler, ImageType},
    input::mouse::{MouseScrollUnit, MouseWheel},
    prelude::*,
    tasks::AsyncComputeTaskPool,
    ui::FocusPolicy,
};
use crossbeam::{Receiver, Sender};
use serde::{Deserialize, Serialize};

use crate::{
    game_state::GameState,
    networking::{ConnectionEvent, Identity},
    settings::Settings,
    singleplayer::SinglePlayerServer,
    ui::{text_input::*, DOUBLE_CLICK_DELAY},
};

use super::{widgets::*, GuiState, Interface, Interfaces, BACKGROUND, BASE_SIZE};

pub const PLAY_BUTTON: Handle<Image> = weak_handle!("ab907f7d-9c04-46c6-b13f-7158a8d90a03");
pub const EDIT_BUTTON: Handle<Image> = weak_handle!("2f9842c2-b1dd-4c8a-aa80-75b8b00f2a88");
pub const DELETE_BUTTON: Handle<Image> = weak_handle!("cdab585e-f900-4c18-ab59-3eecfd1bff79");
pub const LIST_ITEM_TEXTURE: Handle<Image> = weak_handle!("cee5b158-a6f0-4639-ba29-fa9e343768b4");
pub const LIST_ITEM_PLACEHOLDER_IMAGE: Handle<Image> =
    weak_handle!("13d6092a-bcb3-4583-8581-d1701f3463e9");

const CONTENT_WIDTH: Val = Val::Percent(56.0);

pub struct MainMenuPlugin;
impl Plugin for MainMenuPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, (setup, download_default_game))
            // The main menu lists are rebuilt each time entered to react to worlds being deleted or
            // created from other interfaces.
            .add_systems(OnEnter(GuiState::MainMenu), update_lists)
            .add_systems(
                Update,
                (
                    report_game_download_progress,
                    switch_tab,
                    handle_list_item_interactions,
                    handle_main_button_clicks,
                    goto_login,
                    scroll,
                    search,
                )
                    .run_if(in_state(GuiState::MainMenu)),
            )
            .add_systems(OnEnter(GameState::Playing), clear_search);

        let load_image = |bytes: &[u8], _path: String| -> Image {
            Image::from_buffer(
                bytes,
                ImageType::Format(ImageFormat::Png),
                CompressedImageFormats::NONE,
                true,
                ImageSampler::nearest(),
                RenderAssetUsages::RENDER_WORLD,
            )
            .expect("Failed to load image")
        };

        load_internal_binary_asset!(app, PLAY_BUTTON, "../../../assets/ui/play.png", load_image);
        load_internal_binary_asset!(app, EDIT_BUTTON, "../../../assets/ui/edit.png", load_image);
        load_internal_binary_asset!(
            app,
            DELETE_BUTTON,
            "../../../assets/ui/delete.png",
            load_image
        );
        load_internal_binary_asset!(
            app,
            LIST_ITEM_TEXTURE,
            "../../../assets/ui/list_item.png",
            load_image
        );
        load_internal_binary_asset!(
            app,
            LIST_ITEM_PLACEHOLDER_IMAGE,
            "../../../assets/ui/vista.png",
            load_image
        );
    }
}

fn setup(mut commands: Commands, settings: Res<Settings>, mut interfaces: ResMut<Interfaces>) {
    // TODO: The lists should be able to just grow to fill the available space with flex_grow,
    // instead there's no limit to how far they can grow, so it just ends up forcing all the other
    // elements to shrink when the lists grow in element count. It's instead forced to be this
    // tall. Distorting the window out of 16:9 will leave the list not filling the screen.
    let list_height = Val::Px(196.0);

    let entity = commands
        .spawn((
            Interface,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: BASE_SIZE * 15.0,
                ..default()
            },
            ImageNode::new(BACKGROUND),
        ))
        .with_children(|parent| {
            parent.spawn_header();

            // Tabs
            parent
                .spawn(Node {
                    width: Val::Percent(100.0),
                    height: BASE_SIZE * 27.0,
                    justify_content: JustifyContent::SpaceEvenly,
                    ..default()
                })
                .with_children(|parent| {
                    parent
                        .spawn_tab("Singleplayer", Val::Px(133.5), true)
                        .insert(Tabs::Singleplayer);
                    parent
                        .spawn_tab("Multiplayer", Val::Px(133.5), false)
                        .insert(Tabs::Multiplayer);
                });

            // World search bar / new world button
            parent
                .spawn((
                    Node {
                        width: CONTENT_WIDTH,
                        height: BASE_SIZE * 17.0,
                        column_gap: Val::Percent(3.0),
                        ..default()
                    },
                    Tabs::Singleplayer,
                ))
                .with_children(|parent| {
                    parent
                        .spawn_textbox(TextBox::new("Search"))
                        .insert(WorldSearchTextBox);
                    parent
                        .spawn_button(
                            "New World",
                            ButtonStyle {
                                width: BASE_SIZE * 60.0,
                                color: colors::BUTTON_GREEN,
                                ..default()
                            },
                        )
                        .insert(MainButton::NewWorld);
                });

            // World list
            parent
                .spawn((
                    Node {
                        flex_direction: FlexDirection::Column,
                        max_height: list_height,
                        width: CONTENT_WIDTH,
                        overflow: Overflow::scroll(),
                        align_items: AlignItems::Center,
                        row_gap: BASE_SIZE * 4.0,
                        // Moves it above the footer
                        // margin: UiRect::bottom(BASE_SIZE * 6.0),
                        ..default()
                    },
                    WorldList,
                    Tabs::Singleplayer,
                ))
                .with_children(|parent| {
                    WorldList::build(parent, &settings);
                });

            // Server search bar / connect button
            parent
                .spawn((
                    Node {
                        display: Display::None,
                        width: CONTENT_WIDTH,
                        height: BASE_SIZE * 17.0,
                        column_gap: Val::Percent(3.0),
                        ..default()
                    },
                    Tabs::Multiplayer,
                ))
                .with_children(|parent| {
                    parent
                        .spawn_textbox(TextBox::new("Search/Address"))
                        .insert(ServerTextBox);
                    parent
                        .spawn_button(
                            "Connect",
                            ButtonStyle {
                                width: BASE_SIZE * 60.0,
                                color: colors::BUTTON_GREEN,
                                ..default()
                            },
                        )
                        .insert(MainButton::Connect);
                });

            // Server list
            let server_list = ServerList::load(&settings);
            parent
                .spawn((
                    Node {
                        display: Display::None,
                        flex_direction: FlexDirection::Column,
                        max_height: list_height,
                        width: CONTENT_WIDTH,
                        overflow: Overflow::scroll(),
                        align_items: AlignItems::Center,
                        row_gap: BASE_SIZE * 4.0,
                        // margin: UiRect::bottom(BASE_SIZE * 6.0),
                        ..default()
                    },
                    Tabs::Multiplayer,
                ))
                .with_children(|parent| {
                    server_list.build(parent);
                })
                .insert(server_list);

            parent.spawn_footer();
        })
        .id();

    interfaces.insert(GuiState::MainMenu, entity);
}

fn update_lists(
    mut commands: Commands,
    settings: Res<Settings>,
    world_list: Query<Entity, With<WorldList>>,
    server_list: Query<(Entity, &ServerList)>,
) {
    // OnEnter triggers in before PreStartup so the entities won't be available for the first run.
    if let Ok(entity) = world_list.single() {
        commands
            .entity(entity)
            .despawn_related::<Children>()
            .with_children(|parent| {
                WorldList::build(parent, &settings);
            });
    }

    if let Ok((entity, server_list)) = server_list.single() {
        commands
            .entity(entity)
            .despawn_related::<Children>()
            .with_children(|parent| {
                server_list.build(parent);
            });
    }
}

trait MainMenuWidgets {
    fn spawn_list_item<'a>(&'a mut self, text: &str) -> EntityCommands<'a>;
}

impl MainMenuWidgets for ChildSpawnerCommands<'_> {
    fn spawn_list_item<'a>(&'a mut self, text: &str) -> EntityCommands<'a> {
        let mut entity_commands = self.spawn((
            Node {
                width: Val::Percent(100.0),
                height: BASE_SIZE * 27.0,
                flex_direction: FlexDirection::Column,
                ..default()
            },
            Interaction::default(),
            // Cover up layout pixel gaps when resizing
            BackgroundColor::from(Color::srgb_u8(62, 57, 55)),
        ));

        let main_entity = entity_commands.id();

        entity_commands.with_children(|parent| {
            parent
                .spawn(Node {
                    height: BASE_SIZE * 25.0,
                    width: Val::Percent(100.0),
                    ..default()
                })
                .with_children(|parent| {
                    // Borders
                    parent.spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            width: Val::Percent(100.0),
                            height: Val::Percent(100.0),
                            border: UiRect {
                                left: BASE_SIZE,
                                top: BASE_SIZE,
                                ..default()
                            },
                            ..default()
                        },
                        BorderColor::from(Srgba::rgb_u8(102, 97, 95)),
                    ));
                    parent.spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            width: Val::Percent(100.0),
                            height: Val::Percent(100.0),
                            border: UiRect {
                                right: BASE_SIZE,
                                bottom: BASE_SIZE,
                                ..default()
                            },
                            ..default()
                        },
                        BorderColor::from(Srgba::rgb_u8(81, 77, 75)),
                    ));

                    // Content
                    parent
                        .spawn((
                            Node {
                                border: UiRect::all(BASE_SIZE),
                                height: Val::Percent(100.0),
                                width: Val::Percent(100.0),
                                align_items: AlignItems::Center,
                                ..default()
                            },
                            BackgroundColor::from(Color::srgb_u8(62, 57, 55)),
                        ))
                        .with_children(|parent| {
                            parent.spawn((
                                Node {
                                    position_type: PositionType::Absolute,
                                    width: Val::Percent(100.0),
                                    height: Val::Percent(100.0),
                                    ..default()
                                },
                                ImageNode {
                                    image: LIST_ITEM_TEXTURE,
                                    color: Color::srgb_u8(62, 57, 55).mix(&Color::WHITE, 0.07),
                                    ..default()
                                },
                            ));
                            // World preview image
                            parent.spawn((
                                Node {
                                    width: Val::Percent(15.0),
                                    height: Val::Percent(100.0),
                                    ..default()
                                },
                                ImageNode::new(LIST_ITEM_PLACEHOLDER_IMAGE),
                            ));
                            // Text container
                            parent
                                .spawn(Node {
                                    height: Val::Percent(100.0),
                                    margin: UiRect::left(Val::Percent(1.0)),
                                    align_items: AlignItems::Center,
                                    overflow: Overflow::hidden(),
                                    flex_grow: 1.0,
                                    ..default()
                                })
                                .with_children(|parent| {
                                    parent.spawn_text(text);
                                });
                            // Delete button
                            parent
                                .spawn((
                                    Node {
                                        width: BASE_SIZE * 25.0,
                                        height: Val::Percent(100.0),
                                        justify_content: JustifyContent::Center,
                                        align_items: AlignItems::Center,
                                        ..default()
                                    },
                                    FocusPolicy::Block,
                                    Interaction::default(),
                                    ListItemButton::Delete(main_entity),
                                ))
                                .with_children(|parent| {
                                    parent.spawn((
                                        Node {
                                            width: BASE_SIZE * 7.0,
                                            height: BASE_SIZE * 8.0,
                                            ..default()
                                        },
                                        ImageNode::new(DELETE_BUTTON),
                                    ));
                                });
                            // Divider
                            parent.spawn((
                                Node {
                                    height: Val::Percent(60.0),
                                    width: BASE_SIZE,
                                    ..default()
                                },
                                BackgroundColor::from(Srgba::rgb_u8(77, 77, 77)),
                            ));
                            // Edit button
                            parent
                                .spawn((
                                    Node {
                                        width: BASE_SIZE * 25.0,
                                        height: Val::Percent(100.0),
                                        justify_content: JustifyContent::Center,
                                        align_items: AlignItems::Center,
                                        ..default()
                                    },
                                    FocusPolicy::Block,
                                    Interaction::default(),
                                    ListItemButton::Edit(main_entity),
                                ))
                                .with_children(|parent| {
                                    parent.spawn((
                                        Node {
                                            width: BASE_SIZE * 8.0,
                                            height: BASE_SIZE * 8.0,
                                            ..default()
                                        },
                                        ImageNode::new(EDIT_BUTTON),
                                    ));
                                });
                            // Divider
                            parent.spawn((
                                Node {
                                    height: Val::Percent(60.0),
                                    width: BASE_SIZE,
                                    ..default()
                                },
                                BackgroundColor::from(Srgba::rgb_u8(77, 77, 77)),
                            ));
                            // Play button
                            parent
                                .spawn((
                                    Node {
                                        width: BASE_SIZE * 25.0,
                                        height: Val::Percent(100.0),
                                        justify_content: JustifyContent::Center,
                                        align_items: AlignItems::Center,
                                        ..default()
                                    },
                                    FocusPolicy::Block,
                                    Interaction::default(),
                                    ListItemButton::Play(main_entity),
                                ))
                                .with_children(|parent| {
                                    parent.spawn((
                                        Node {
                                            width: BASE_SIZE * 4.0,
                                            height: BASE_SIZE * 8.0,
                                            ..default()
                                        },
                                        ImageNode::new(PLAY_BUTTON),
                                    ));
                                });
                        });
                });

            parent.spawn((
                Node {
                    height: BASE_SIZE * 2.0,
                    width: Val::Percent(100.0),
                    ..default()
                },
                BackgroundColor::from(Color::srgb_u8(52, 52, 52)),
            ));
        });

        return entity_commands;
    }
}

#[derive(Component)]
struct WorldSearchTextBox;

#[derive(Component)]
struct WorldList;

impl WorldList {
    fn build(parent: &mut ChildSpawnerCommands, settings: &Settings) {
        for path in Self::read_worlds(settings) {
            parent
                .spawn_list_item(path.file_stem().unwrap().to_str().unwrap())
                .insert(ListItem::World(path));
        }
    }

    fn read_worlds(settings: &Settings) -> Vec<PathBuf> {
        let path = settings.data_dir().join("worlds");

        if let Err(e) = std::fs::create_dir_all(&path) {
            error!("Could not create directory for worlds: {e}");
            return Vec::new();
        }
        let dir = match std::fs::read_dir(&path) {
            Ok(d) => d,
            Err(e) => {
                error!("Could not read from worlds directory: {e}");
                return Vec::new();
            }
        };

        let mut result: Vec<(PathBuf, std::time::SystemTime)> = Vec::new();
        for entry in dir {
            let Ok(entry) = entry else {
                continue;
            };

            let world_path = entry.path();
            if !world_path.extension().is_some_and(|e| e == "sqlite") {
                continue;
            }

            if let Ok(modified) = entry.metadata().and_then(|m| m.modified()) {
                result.push((world_path, modified))
            } else {
                result.push((world_path, std::time::SystemTime::UNIX_EPOCH));
            }
        }

        result.sort_by_key(|(_, time)| *time);
        result.reverse();
        result.into_iter().map(|(path, _)| path).collect()
    }
}

#[derive(Component)]
struct ServerTextBox;

#[derive(Serialize, Deserialize)]
struct Server {
    address: String,
    // TODO: Not implemented
    favourite: bool,
}

#[derive(Component, Serialize, Deserialize)]
struct ServerList {
    servers: Vec<Server>,
}

impl ServerList {
    fn load(settings: &Settings) -> Self {
        let default = Self {
            servers: Vec::new(),
        };

        let path = settings.config_dir().join("servers.json");
        let file = match std::fs::File::open(path) {
            Ok(f) => f,
            Err(e) if e.kind() != ErrorKind::NotFound => {
                error!("Failed to open servers.json: {e}");
                return default;
            }
            _ => return default,
        };

        return match serde_json::from_reader(file) {
            Ok(c) => c,
            Err(e) => {
                error!("Failed to read server list: {}", e);
                default
            }
        };
    }

    fn save(&self, settings: &Settings) {
        let path = settings.config_dir().join("servers.json");
        let file = match std::fs::File::create(path) {
            Ok(f) => f,
            Err(e) => {
                error!("Failed to create servers.json: {e}");
                return;
            }
        };

        if let Err(e) = serde_json::to_writer_pretty(file, self) {
            error!("Failed to write servers.json: {e}");
        };
    }

    fn build(&self, parent: &mut ChildSpawnerCommands) {
        for (index, server) in self.servers.iter().enumerate() {
            parent
                .spawn_list_item(&server.address)
                .insert(ListItem::Server(index));
        }
    }
}

#[derive(Component, PartialEq, Clone, Copy)]
enum Tabs {
    Singleplayer,
    Multiplayer,
}

#[derive(Component)]
enum ListItemButton {
    // Stores the entity of the list item it is part of.
    Play(Entity),
    Edit(Entity),
    Delete(Entity),
}

#[derive(Component, PartialEq)]
enum MainButton {
    NewWorld,
    Connect,
}

fn scroll(
    mut mouse_wheel: EventReader<MouseWheel>,
    tabs: Query<(&Node, &Tabs), Without<Interaction>>,
    mut world_list: Query<&mut ScrollPosition, (With<WorldList>, Without<ServerList>)>,
    mut server_list: Query<&mut ScrollPosition, With<ServerList>>,
) {
    for mouse_wheel_event in mouse_wheel.read() {
        let mut open_tab = Tabs::Singleplayer;
        for (node, tab) in tabs.iter() {
            if node.display != Display::None {
                open_tab = *tab;
                break;
            }
        }

        let dy = match mouse_wheel_event.unit {
            MouseScrollUnit::Line => mouse_wheel_event.y * 24.0,
            MouseScrollUnit::Pixel => mouse_wheel_event.y,
        };

        let mut scroll_position = if open_tab == Tabs::Singleplayer {
            world_list.single_mut().unwrap()
        } else {
            server_list.single_mut().unwrap()
        };

        scroll_position.offset_y -= dy;
    }
}

fn clear_search(
    mut commands: Commands,
    settings: Res<Settings>,
    asset_server: Res<AssetServer>,
    mut world_search_bar: Query<&mut TextBox, (With<WorldSearchTextBox>, Without<ServerTextBox>)>,
    mut server_search_bar: Query<(&mut TextBox, &ChildOf), With<ServerTextBox>>,
    tab_content: Query<&Node, With<Tabs>>,
    mut server_list: Query<(Entity, &mut ServerList)>,
) {
    world_search_bar.single_mut().unwrap().text.clear();

    // To not litter the server list with incorrect addresses, they are only added after a
    // successful connection.
    let (text_box, parent) = &mut server_search_bar.single_mut().unwrap();
    // Player might type into the server search bar only to switch to singleplayer and connect that
    // way. Check that the multiplayer tab was the one that was open when connecting so we don't
    // add the incomplete search as a server entry.
    if !text_box.text.is_empty() && tab_content.get(parent.0).unwrap().display != Display::None {
        let (entity, mut server_list) = server_list.single_mut().unwrap();
        for server in server_list.servers.iter() {
            if &text_box.text == &server.address {
                return;
            }
        }

        server_list.servers.insert(
            0,
            Server {
                address: text_box.text.clone(),
                favourite: false,
            },
        );
        server_list.save(&settings);
    }
    text_box.text.clear();
}

fn search(
    world_search_bar: Query<
        &TextBox,
        (
            Or<(Changed<TextBox>, Changed<InheritedVisibility>)>,
            With<WorldSearchTextBox>,
        ),
    >,
    server_search_bar: Query<
        &TextBox,
        (
            Or<(Changed<TextBox>, Changed<InheritedVisibility>)>,
            With<ServerTextBox>,
        ),
    >,
    server_list: Query<&ServerList>,
    mut worlds_and_servers: Query<(&mut Node, &ListItem)>,
) {
    // TODO: Unicode
    fn case_insensitive_search(haystack: &str, needle: &str) -> bool {
        if needle.is_empty() {
            return true;
        }

        let haystack_bytes = haystack.as_bytes();
        let needle_bytes = needle.as_bytes();

        for i in 0..=(haystack_bytes.len().saturating_sub(needle_bytes.len())) {
            let mut match_found = true;

            if needle_bytes.len() > haystack_bytes.len() - i {
                break;
            }

            for j in 0..needle_bytes.len() {
                let haystack_char = haystack_bytes[i + j];
                let needle_char = needle_bytes[j];

                if haystack_char.to_ascii_lowercase() != needle_char.to_ascii_lowercase() {
                    match_found = false;
                    break;
                }
            }

            if match_found {
                return true;
            }
        }

        false
    }

    if let Ok(textbox) = world_search_bar.single() {
        for (mut node, list_item) in worlds_and_servers.iter_mut() {
            let ListItem::World(path) = list_item else {
                continue;
            };

            let name = path.file_name().unwrap().to_str().unwrap();
            if case_insensitive_search(name, &textbox.text) {
                node.display = Display::Flex;
            } else {
                node.display = Display::None;
            }
        }
    };

    if let Ok(textbox) = server_search_bar.single() {
        for (mut node, list_item) in worlds_and_servers.iter_mut() {
            let ListItem::Server(index) = list_item else {
                continue;
            };

            let server_list = server_list.single().unwrap();
            let server = &server_list.servers[*index];

            if case_insensitive_search(&server.address, &textbox.text) {
                node.display = Display::Flex;
            } else {
                node.display = Display::None;
            }
        }
    };
}

fn handle_list_item_interactions(
    mut commands: Commands,
    settings: Res<Settings>,
    mut singleplayer_server: ResMut<SinglePlayerServer>,
    mut gui_state: ResMut<NextState<GuiState>>,
    mut configured_world: ResMut<super::world_configuration::ConfiguredWorld>,
    list_items: Query<(Entity, Ref<Interaction>, &ListItem)>,
    mut server_list: Query<(Entity, &mut ServerList)>,
    button_clicks: Query<(&Interaction, &ListItemButton), Changed<Interaction>>,
    mut connection_events: EventWriter<ConnectionEvent>,
    mut last_click: Local<Option<(Entity, std::time::Instant)>>,
) {
    for (list_item_entity, interaction, list_item) in list_items.iter() {
        if !interaction.is_changed() {
            continue;
        }

        let (last_list_item, last_click) =
            last_click.get_or_insert((Entity::PLACEHOLDER, std::time::Instant::now()));

        if *interaction == Interaction::Pressed {
            if last_click.elapsed().as_secs_f32() < DOUBLE_CLICK_DELAY
                && list_item_entity == *last_list_item
            {
                match list_item {
                    ListItem::World(path) => {
                        // The worlds are ordered based on their last modification
                        if let Ok(file) = std::fs::File::open(path) {
                            file.set_modified(std::time::SystemTime::now());
                        }

                        singleplayer_server.start(&path);
                    }
                    ListItem::Server(index) => {
                        let (_, mut server_list) = server_list.single_mut().unwrap();
                        let server = server_list.servers.remove(*index);
                        connection_events.write(ConnectionEvent {
                            address: server.address.clone(),
                        });
                        server_list.servers.insert(0, server);
                    }
                }
            } else {
                *last_click = std::time::Instant::now();
                *last_list_item = list_item_entity;
            }
        }
    }

    'outer: for (interaction, button_type) in button_clicks.iter() {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match button_type {
            ListItemButton::Play(list_item_entity) => {
                let (_, _, list_item) = list_items.get(*list_item_entity).unwrap();
                match list_item {
                    ListItem::World(path) => {
                        // The worlds are ordered based on their last modification
                        if let Ok(file) = std::fs::File::open(path) {
                            file.set_modified(std::time::SystemTime::now());
                        }

                        singleplayer_server.start(&path);
                    }
                    ListItem::Server(index) => {
                        let (_, mut server_list) = server_list.single_mut().unwrap();
                        let server = server_list.servers.remove(*index);
                        connection_events.write(ConnectionEvent {
                            address: server.address.clone(),
                        });
                        server_list.servers.insert(0, server);
                    }
                }
            }
            ListItemButton::Edit(list_item_entity) => {
                let (_, _, list_item) = list_items.get(*list_item_entity).unwrap();
                match list_item {
                    ListItem::World(path) => {
                        configured_world.edit(path);
                        gui_state.set(GuiState::WorldConfiguration);
                    }
                    _ => (),
                }
            }
            ListItemButton::Delete(list_item_entity) => {
                let (_, _, list_item) = list_items.get(*list_item_entity).unwrap();

                match list_item {
                    ListItem::World(path) => {
                        commands.entity(*list_item_entity).despawn();

                        if let Err(e) = std::fs::remove_file(path) {
                            warn!("Encountered error when deleting a world: {e}");
                        };
                    }
                    ListItem::Server(index) => {
                        let (entity, mut server_list) = server_list.single_mut().unwrap();
                        server_list.servers.remove(*index);
                        server_list.save(&settings);
                        commands
                            .entity(entity)
                            .despawn_related::<Children>()
                            .with_children(|parent| server_list.build(parent));
                    }
                }
            }
        }
    }
}

fn handle_main_button_clicks(
    mut gui_state: ResMut<NextState<GuiState>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut configured_world: ResMut<super::world_configuration::ConfiguredWorld>,
    buttons: Query<(&Interaction, &MainButton, &InheritedVisibility)>,
    server_input: Query<&TextBox, With<ServerTextBox>>,
    mut connection_events: EventWriter<ConnectionEvent>,
) {
    for (interaction, button, visibility) in buttons.iter() {
        let connect_with_enter =
            keys.just_pressed(KeyCode::Enter) && *button == MainButton::Connect && visibility.get();
        if *interaction != Interaction::Pressed && !connect_with_enter {
            continue;
        }

        match button {
            MainButton::NewWorld => {
                if Path::new("fmc_server/server").exists() {
                    configured_world.new_world();
                    gui_state.set(GuiState::WorldConfiguration);
                }
            }
            MainButton::Connect => {
                let address = server_input.single().unwrap().text.clone();
                if address.is_empty() {
                    continue;
                }

                // The server is added as a list item in clear_search if it successfully connects
                connection_events.write(ConnectionEvent {
                    address: address.clone(),
                });
            }
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

#[derive(Component)]
enum ListItem {
    // Holds an index into the ServerList
    Server(usize),
    // The path to a world folder
    World(PathBuf),
}

enum DownloadStatus {
    Success,
    Progress { current: usize, total: usize },
    Failure(String),
}

#[derive(Component)]
struct DownloadReporter(Receiver<DownloadStatus>);

fn report_game_download_progress(
    mut commands: Commands,
    time: Res<Time>,
    mut status_text: Query<&mut TextBox, With<WorldSearchTextBox>>,
    downloads: Query<(Entity, &DownloadReporter)>,
    mut timer: Local<Timer>,
) {
    let mut text_box = status_text.single_mut().unwrap();

    for (download_entity, reporter) in downloads.iter() {
        while let Ok(status) = reporter.0.try_recv() {
            match status {
                DownloadStatus::Success => {
                    *timer = Timer::from_seconds(2.0, TimerMode::Once);
                    commands.entity(download_entity).despawn();
                    //text_box.placeholder_text = "Singleplayer server downloaded!";
                }
                DownloadStatus::Progress { current, total } => {
                    fn bytes_to_string(bytes: usize) -> String {
                        const UNITS: [&str; 6] = ["B", "KB", "MB", "GB", "TB", "PB"];

                        let mut index = 0;
                        let mut value = bytes as f64;

                        while value >= 1024.0 && index < UNITS.len() - 1 {
                            value /= 1024.0;
                            index += 1;
                        }

                        // Round to one decimal place
                        let rounded_value = (value * 10.0).round() / 10.0;

                        // Format the result
                        format!("{:.1}{}", rounded_value, UNITS[index])
                    }

                    text_box.placeholder_text = format!(
                        "Downloading singleplayer: {}/{}",
                        bytes_to_string(current),
                        bytes_to_string(total)
                    );
                }
                DownloadStatus::Failure(err) => {
                    error!(err);
                    commands.entity(download_entity).despawn();
                    text_box.placeholder_text = "Failed to download singleplayer server".to_owned();
                    // Let the failure text linger to make sure the player sees it
                    *timer = Timer::from_seconds(10000.0, TimerMode::Once);
                }
            }
        }
    }

    timer.tick(time.delta());
    if timer.just_finished() {
        text_box.placeholder_text = "Search".to_owned();
    }
}

fn download_default_game(mut commands: Commands) {
    let server_folder = PathBuf::from("fmc_server");
    let server_path = server_folder.join("server".to_owned() + std::env::consts::EXE_SUFFIX);
    if server_path.exists() {
        return;
    }

    let url = match (std::env::consts::OS, std::env::consts::ARCH) {
        ("linux", "x86_64") => "https://github.com/awowogei/fmc_173/releases/download/nightly/x86_64-unknown-linux-gnu",
        ("windows", "x86_64") => "https://github.com/awowogei/fmc_173/releases/download/nightly/x86_64-pc-windows-msvc.exe",
        ("macos", "x86_64") => "https://github.com/awowogei/fmc_173/releases/download/nightly/x86_64-apple-darwin",
        ("macos", "aarch64") => "https://github.com/awowogei/fmc_173/releases/download/nightly/aarch64-apple-darwin",
        _ => return
    }.to_owned();

    let (sender, receiver) = crossbeam::unbounded();
    commands.spawn(DownloadReporter(receiver));

    AsyncComputeTaskPool::get()
        .spawn(download_game(url, server_folder, sender))
        .detach();
}

async fn download_game(url: String, download_folder: PathBuf, reporter: Sender<DownloadStatus>) {
    if !download_folder.exists() {
        if let Err(e) = std::fs::create_dir(&download_folder) {
            reporter
                .send(DownloadStatus::Failure(format!(
                    "Could not create download directory: {}",
                    e
                )))
                .unwrap();
            return;
        }
    };

    let Ok(response) = ureq::get(&url).call() else {
        reporter
            .send(DownloadStatus::Failure(
                "Download url inaccessible".to_owned(),
            ))
            .unwrap();
        return;
    };

    if response.status() != 200 {
        reporter
            .send(DownloadStatus::Failure(
                "Download refused by server".to_owned(),
            ))
            .unwrap();
        return;
    }

    let temp_path = download_folder.join("server_temp");
    let file = match std::fs::File::create(&temp_path) {
        Ok(f) => f,
        Err(e) => {
            reporter
                .send(DownloadStatus::Failure(format!(
                    "Could not create download file: {}",
                    e
                )))
                .unwrap();
            return;
        }
    };
    let mut file = std::io::BufWriter::new(file);

    let size = response.headers()["content-length"]
        .to_str()
        .unwrap()
        .parse()
        .unwrap();
    let mut reader = response.into_body().into_reader();
    let mut downloaded = 0;
    loop {
        let mut buf = vec![0; 2048];
        match reader.read(&mut buf) {
            Ok(n) if n > 0 => {
                // clone data from buffer and clear it
                let Ok(written) = file.write(&buf[..n]) else {
                    reporter
                        .send(DownloadStatus::Failure(
                            "File unexpectedly unavailable".to_owned(),
                        ))
                        .unwrap();
                    return;
                };
                downloaded += written;
                reporter
                    .send(DownloadStatus::Progress {
                        current: downloaded,
                        total: size,
                    })
                    .unwrap();
            }
            Ok(_) => {
                if file.flush().is_err() {
                    panic!();
                };

                let final_path =
                    download_folder.join("server".to_owned() + std::env::consts::EXE_SUFFIX);

                if let Err(e) = std::fs::rename(&temp_path, &final_path) {
                    warn!("Couldn't rename server executable, {e}");
                }

                if std::env::consts::FAMILY == "unix" {
                    if std::process::Command::new("chmod")
                        .arg("+x")
                        .arg(&final_path)
                        .status()
                        .is_err()
                    {
                        error!("Couldn't set execution permissions for server");
                    }
                }

                reporter.send(DownloadStatus::Success).unwrap();
                return;
            }
            Err(err) => {
                reporter
                    .send(DownloadStatus::Failure(err.to_string()))
                    .unwrap();
                return;
            }
        };
    }
}

fn goto_login(identity: Res<Identity>, mut gui_state: ResMut<NextState<GuiState>>) {
    if !identity.is_valid() {
        gui_state.set(GuiState::Login);
    }
}
