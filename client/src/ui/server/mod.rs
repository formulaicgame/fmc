use std::collections::HashMap;

use bevy::{
    ecs::system::EntityCommands,
    input::keyboard::{Key, KeyboardInput},
    prelude::*,
    render::{
        render_asset::RenderAssetUsages,
        texture::{CompressedImageFormats, ImageSampler},
    },
    window::{CursorGrabMode, PrimaryWindow},
};
use fmc_networking::{messages, NetworkClient, NetworkData};
use serde::Deserialize;

use crate::{game_state::GameState, ui::widgets::TextBox};

use self::items::{CursorItemBox, ItemBoxSection};
use super::widgets::Widgets;

pub mod items;
pub mod key_bindings;
mod textbox;

const INTERFACE_CONFIG_PATH: &str = "server_assets/interfaces/";
const INTERFACE_TEXTURE_PATH: &str = "server_assets/textures/interfaces/";

pub struct ServerInterfacesPlugin;
impl Plugin for ServerInterfacesPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<InterfaceToggleEvent>()
            .insert_resource(InterfaceStack::default())
            .insert_resource(KeyboardFocus::default())
            .add_plugins((
                items::ItemPlugin,
                textbox::TextBoxPlugin,
                key_bindings::KeyBindingsPlugin,
            ))
            .add_systems(
                Update,
                (
                    handle_visibility_updates,
                    handle_toggle_events,
                    handle_open_request,
                    handle_close_request,
                    button_interaction,
                    cursor_visibility,
                    handle_escape_key,
                )
                    .run_if(GameState::in_game),
            );
    }
}

#[derive(Component)]
pub struct InterfaceNode {
    pub path: String,
}

#[derive(Resource, Deref, DerefMut, Default)]
pub struct InterfacePaths(HashMap<String, Vec<Entity>>);

// A map from 'InterfacePath' to entity.
#[derive(Resource, Deref, DerefMut, Default)]
pub struct Interfaces(HashMap<String, Entity>);

// Called when loading assets.
pub fn load_interfaces(
    mut commands: Commands,
    net: Res<NetworkClient>,
    asset_server: Res<AssetServer>,
) {
    let mut interfaces = Interfaces::default();
    let mut interface_paths = InterfacePaths::default();

    let directory = match std::fs::read_dir(INTERFACE_CONFIG_PATH) {
        Ok(dir) => dir,
        Err(e) => {
            net.disconnect(&format!(
                "Misconfigured resource pack: Failed to read interface configuration directory '{}'\n\
                Error: {}",
                INTERFACE_CONFIG_PATH, e
            ));
            return;
        }
    };

    for dir_entry in directory {
        let file_path = match dir_entry {
            Ok(d) => d.path(),
            Err(e) => {
                net.disconnect(&format!(
                    "Failed to read the file path of an interface config\nError: {}",
                    e
                ));
                return;
            }
        };

        let file = match std::fs::File::open(&file_path) {
            Ok(f) => f,
            Err(e) => {
                net.disconnect(&format!(
                    "Failed to open interface configuration at: '{}'\nError: {}",
                    &file_path.display(),
                    e
                ));
                return;
            }
        };

        let node_config: NodeConfig = match serde_json::from_reader(&file) {
            Ok(c) => c,
            Err(e) => {
                net.disconnect(&format!(
                "Misconfigured resource pack: Failed to read interface configuration at: '{}'\n\
                Error: {}",
                &file_path.display(),
                e
            ));
                return;
            }
        };

        // NOTE(WORKAROUND): When spawning an ImageBundle, the dimensions of the image are
        // inferred, but if it has children, it's discarded and it uses the size of the children
        // instead. Images must therefore be spawned with defined width/height to display correctly.
        fn read_image_dimensions(image_path: &str) -> Vec2 {
            let image_data = match std::fs::read(INTERFACE_TEXTURE_PATH.to_owned() + image_path) {
                Ok(i) => i,
                Err(_) => {
                    return Vec2::ZERO;
                }
            };

            let image = match Image::from_buffer(
                &image_data,
                bevy::render::texture::ImageType::Extension("png"),
                CompressedImageFormats::NONE,
                false,
                ImageSampler::Default,
                RenderAssetUsages::default(),
            ) {
                Ok(i) => i,
                Err(_) => {
                    return Vec2::ZERO;
                }
            };

            return image.size_f32();
        }

        // TODO: The server needs to validate that no interfaces share a name. The client doesn't
        // need to care, it will just overwrite. It is hard to do with this recursion too.
        fn spawn_interface(
            entity_commands: &mut EntityCommands,
            parent_path: String,
            config: &NodeConfig,
            interface_paths: &mut InterfacePaths,
            asset_server: &AssetServer,
        ) {
            let node_path = if let Some(path) = &config.path {
                let node_path = if parent_path == "" {
                    path.to_owned()
                } else {
                    parent_path + "/" + path
                };

                entity_commands.insert(InterfaceNode {
                    path: node_path.clone(),
                });

                interface_paths
                    .entry(node_path.clone())
                    .or_default()
                    .push(entity_commands.id());

                node_path
            } else {
                parent_path
            };

            let style = if let Some(image_path) = &config.image {
                let dimensions = read_image_dimensions(&image_path);
                let mut style = Style::from(config.style.clone());
                style.width = Val::Px(dimensions.x);
                style.height = Val::Px(dimensions.y);
                style
            } else {
                config.style.clone().into()
            };

            let background_color = if let Some(background_color) = config.background_color {
                background_color
            } else if config.image.is_some() {
                Color::WHITE
            } else {
                Color::NONE
            };

            entity_commands.insert((
                NodeBundle {
                    style: style.clone(),
                    background_color: background_color.into(),
                    border_color: config.border_color.unwrap_or(Color::NONE).into(),
                    ..default()
                },
                config.image.as_ref().map_or(UiImage::default(), |path| {
                    asset_server
                        .load(INTERFACE_TEXTURE_PATH.to_owned() + &path)
                        .into()
                }),
            ));

            match &config.content {
                NodeContent::Nodes(nodes) => {
                    entity_commands.with_children(|parent| {
                        for child_config in nodes.iter() {
                            let mut parent_entity_commands = parent.spawn_empty();
                            spawn_interface(
                                &mut parent_entity_commands,
                                node_path.clone(),
                                child_config,
                                interface_paths,
                                asset_server,
                            )
                        }
                    });
                }
                NodeContent::Items(section) => {
                    entity_commands.insert(section.clone());
                }
                NodeContent::Button(nodes) => {
                    entity_commands.insert((Interaction::default(), Button));
                    entity_commands.with_children(|parent| {
                        for child_config in nodes.iter() {
                            let mut parent_entity_commands = parent.spawn_empty();
                            spawn_interface(
                                &mut parent_entity_commands,
                                node_path.clone(),
                                child_config,
                                interface_paths,
                                asset_server,
                            )
                        }
                    });
                }
                NodeContent::TextBox {
                    input: is_input,
                    scrollable,
                    text_background_color,
                    fade,
                } => {
                    entity_commands.insert(TextBox {
                        is_input: *is_input,
                        scrollable: *scrollable,
                        text_background_color: text_background_color.unwrap_or(Color::NONE),
                        ..default()
                    });

                    if *fade {
                        entity_commands.insert(textbox::FadeLines);
                    }
                }
                NodeContent::Text {
                    text,
                    font_size,
                    color,
                } => {
                    entity_commands.with_children(|parent| {
                        parent.spawn_text(
                            text,
                            *font_size,
                            *color,
                            style.flex_direction,
                            style.justify_content,
                            style.align_items,
                        );
                    });
                }
                NodeContent::None => (),
            }
        }

        let interface_entity = commands
            .spawn(NodeBundle {
                style: Style {
                    position_type: PositionType::Absolute,
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                ..default()
            })
            .with_children(|parent| {
                let mut entity_commands = parent.spawn_empty();
                spawn_interface(
                    &mut entity_commands,
                    String::new(),
                    &node_config,
                    &mut interface_paths,
                    &asset_server,
                );

                entity_commands.insert(());
            })
            .insert((
                InterfaceConfig {
                    is_exclusive: node_config.exclusive,
                    keyboard_focus: node_config.keyboard_focus,
                },
                VisibilityBundle {
                    visibility: Visibility::Hidden,
                    ..default()
                },
            ))
            .id();

        // (Probably) safe to unwrap here, as it has already loaded a file with the name.
        let interface_name = file_path.file_stem().unwrap().to_string_lossy().to_string();

        interfaces.insert(interface_name, interface_entity);
    }

    commands.insert_resource(interface_paths);
    commands.insert_resource(interfaces);

    commands
        .spawn((
            ImageBundle {
                style: Style {
                    width: Val::Px(14.0),
                    height: Val::Px(15.8),
                    position_type: PositionType::Absolute,
                    flex_direction: FlexDirection::ColumnReverse,
                    align_items: AlignItems::FlexEnd,
                    ..default()
                },
                z_index: ZIndex::Global(1),
                ..default()
            },
            CursorItemBox::default(),
        ))
        .with_children(|parent| {
            parent.spawn(TextBundle {
                style: Style {
                    top: Val::Px(1.0),
                    left: Val::Px(2.0),
                    ..default()
                },
                ..default()
            });
        });
}

/// Event used by keybindings to toggle an interface open or closed.
#[derive(Event)]
pub struct InterfaceToggleEvent {
    pub interface_entity: Entity,
}
#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct NodeConfig {
    /// Optional name, the server uses this when referring to an interfaces.
    path: Option<String>,
    /// Style used to render the interface node.
    style: NodeStyle,
    /// Content contained by the node.
    content: NodeContent,
    /// Image displayed in the node.
    image: Option<String>,
    /// Fill color, can be used to tint image
    background_color: Option<Color>,
    /// Color used for borders
    border_color: Option<Color>,
    /// If it should overlap(false) or replace(true) interfaces when opened, only
    /// applicable to interface roots.
    exclusive: bool,
    /// If the interface should take keyboard focus, only applicable to interface roots.
    keyboard_focus: KeyboardFocus,
}

#[derive(Component)]
struct InterfaceConfig {
    is_exclusive: bool,
    keyboard_focus: KeyboardFocus,
}

// The current focus is stored as a resource
#[derive(Resource, Deserialize, Default, PartialEq, Clone, Copy)]
enum KeyboardFocus {
    // Keyboard focus is not taken
    #[default]
    None,
    // Takes away movement, other keys work
    Movement,
    // Interface consumes all keyboard input, only Escape will close it.
    Full,
}

// TODO: As json I want it to be like "content: [..]" for Nodes, but all others should be
// adjacently tagged like "content: {type: item_box_section, fields...}". Maybe the adjacently
// tagged ones need to be another enum that is wrapped by one of this enums variants.
#[derive(Default, Deserialize)]
enum NodeContent {
    #[default]
    None,
    // Layout nodes
    Nodes(Vec<NodeConfig>),
    // Node the server can fill with items stacks.
    Items(ItemBoxSection),
    // Customizable button that has its interactions sent to the server.
    Button(Vec<NodeConfig>),
    // Dual use text container, can be filled with text by the server, or used as an input field.
    TextBox {
        #[serde(default)]
        input: bool,
        #[serde(default)]
        scrollable: bool,
        // TODO: Maybe this should be part of the InterfaceTextBoxUpdate message instead, so that
        // you can have individual colors for each line.
        //
        // If you do not want the textbox itself to have color, this allows you to set the
        // background color of the lines themselves.
        text_background_color: Option<Color>,
        // If true, new lines will be set to visible when received and then faded out after a short
        // interval.
        #[serde(default)]
        fade: bool,
    },
    // A predefined text field
    Text {
        text: String,
        font_size: f32,
        color: Color,
    },
}

#[derive(Deserialize, Default, Clone, Debug)]
#[serde(default)]
struct Rect {
    left: Option<Val>,
    right: Option<Val>,
    top: Option<Val>,
    bottom: Option<Val>,
}

impl From<Rect> for UiRect {
    fn from(value: Rect) -> Self {
        UiRect {
            left: value.left.unwrap_or(Val::Px(0.0)),
            right: value.right.unwrap_or(Val::Px(0.0)),
            top: value.top.unwrap_or(Val::Px(0.0)),
            bottom: value.bottom.unwrap_or(Val::Px(0.0)),
        }
    }
}

// TODO: Maybe open issue in bevy see if Style can be made de/se. Missing for UiRect, but
// the rest have it.
//
// Wrapper for 'Style' that is deserializable
#[derive(Deserialize, Clone, Debug)]
#[serde(default, deny_unknown_fields)]
struct NodeStyle {
    pub display: Display,
    pub position_type: PositionType,
    pub overflow: Overflow,
    pub direction: Direction,
    pub left: Val,
    pub right: Val,
    pub top: Val,
    pub bottom: Val,
    pub width: Val,
    pub height: Val,
    pub min_width: Val,
    pub min_height: Val,
    pub max_width: Val,
    pub max_height: Val,
    pub aspect_ratio: Option<f32>,
    pub align_items: AlignItems,
    pub justify_items: JustifyItems,
    pub align_self: AlignSelf,
    pub justify_self: JustifySelf,
    pub align_content: AlignContent,
    pub justify_content: JustifyContent,
    pub margin: Rect,
    pub padding: Rect,
    pub border: Rect,
    pub flex_direction: FlexDirection,
    pub flex_wrap: FlexWrap,
    pub flex_grow: f32,
    pub flex_shrink: f32,
    pub flex_basis: Val,
    pub row_gap: Val,
    pub column_gap: Val,
    pub grid_auto_flow: GridAutoFlow,
    pub grid_template_rows: Vec<RepeatedGridTrack>,
    pub grid_template_columns: Vec<RepeatedGridTrack>,
    pub grid_auto_rows: Vec<GridTrack>,
    pub grid_auto_columns: Vec<GridTrack>,
    pub grid_row: GridPlacement,
    pub grid_column: GridPlacement,
}

impl Default for NodeStyle {
    fn default() -> Self {
        Self {
            display: Display::DEFAULT,
            position_type: PositionType::default(),
            left: Val::Auto,
            right: Val::Auto,
            top: Val::Auto,
            bottom: Val::Auto,
            direction: Direction::default(),
            flex_direction: FlexDirection::default(),
            flex_wrap: FlexWrap::default(),
            align_items: AlignItems::default(),
            justify_items: JustifyItems::DEFAULT,
            align_self: AlignSelf::DEFAULT,
            justify_self: JustifySelf::DEFAULT,
            align_content: AlignContent::DEFAULT,
            justify_content: JustifyContent::DEFAULT,
            margin: Rect::default(),
            padding: Rect::default(),
            border: Rect::default(),
            flex_grow: 0.0,
            flex_shrink: 1.0,
            flex_basis: Val::Auto,
            width: Val::Auto,
            height: Val::Auto,
            min_width: Val::Auto,
            min_height: Val::Auto,
            max_width: Val::Auto,
            max_height: Val::Auto,
            aspect_ratio: None,
            overflow: Overflow::DEFAULT,
            row_gap: Val::Px(0.0),
            column_gap: Val::Px(0.0),
            grid_auto_flow: GridAutoFlow::default(),
            grid_template_rows: Vec::new(),
            grid_template_columns: Vec::new(),
            grid_auto_rows: Vec::new(),
            grid_auto_columns: Vec::new(),
            grid_column: GridPlacement::default(),
            grid_row: GridPlacement::default(),
        }
    }
}

impl From<NodeStyle> for Style {
    fn from(value: NodeStyle) -> Self {
        Style {
            display: value.display,
            position_type: value.position_type,
            overflow: value.overflow,
            direction: value.direction,
            left: value.left,
            right: value.right,
            top: value.top,
            bottom: value.bottom,
            width: value.width,
            height: value.height,
            min_width: value.min_width,
            min_height: value.min_height,
            max_width: value.max_width,
            max_height: value.max_height,
            aspect_ratio: value.aspect_ratio,
            align_items: value.align_items,
            justify_items: value.justify_items,
            align_self: value.align_self,
            justify_self: value.justify_self,
            align_content: value.align_content,
            justify_content: value.justify_content,
            margin: value.margin.into(),
            padding: value.padding.into(),
            border: value.border.into(),
            flex_direction: value.flex_direction,
            flex_wrap: value.flex_wrap,
            flex_grow: value.flex_grow,
            flex_shrink: value.flex_shrink,
            flex_basis: value.flex_basis,
            row_gap: value.row_gap,
            column_gap: value.column_gap,
            grid_auto_flow: value.grid_auto_flow,
            grid_template_rows: value.grid_template_rows,
            grid_template_columns: value.grid_template_columns,
            grid_auto_rows: value.grid_auto_rows,
            grid_auto_columns: value.grid_auto_columns,
            grid_row: value.grid_row,
            grid_column: value.grid_column,
        }
    }
}

// Interfaces that are open and allow other interfaces to take focus are stored here while the
// focused one is visible.
#[derive(Resource, Deref, DerefMut, Default)]
struct InterfaceStack(Vec<Entity>);

fn cursor_visibility(
    mut window: Query<&mut Window, With<PrimaryWindow>>,
    changed_interfaces: Query<(&Visibility, &InterfaceConfig), Changed<Visibility>>,
) {
    for (visibility, config) in changed_interfaces.iter() {
        if config.is_exclusive {
            let mut window = window.single_mut();

            if visibility == Visibility::Visible {
                window.cursor.visible = true;
                let position = Vec2::new(window.width() / 2.0, window.height() / 2.0);
                window.set_cursor_position(Some(position));
                window.cursor.grab_mode = CursorGrabMode::None;
            } else {
                window.cursor.visible = false;
                window.cursor.grab_mode = if cfg!(unix) {
                    CursorGrabMode::Locked
                } else {
                    CursorGrabMode::Confined
                };
            }
        }
    }
}

// TODO: Interaction isn't granular enough. Buttons should only be pressed when they are hovered
// over and the button is released. https://github.com/bevyengine/bevy/pull/9240 is fix I think.
// Remember to also change it for the client GUI buttons, it will solve the problem
// of mouse button spillover. Currently it plays the item use animation when you come out of the
// pause menu.
fn button_interaction(
    net: Res<NetworkClient>,
    button_query: Query<(&Interaction, &InterfaceNode), (Changed<Interaction>, With<Button>)>,
) {
    for (interaction, interface_node) in button_query.iter() {
        match *interaction {
            Interaction::Pressed => net.send_message(messages::InterfaceInteraction::Button {
                interface_path: interface_node.path.clone(),
            }),
            _ => (),
        }
    }
}

fn handle_visibility_updates(
    net: Res<NetworkClient>,
    interface_paths: Res<InterfacePaths>,
    mut interface_query: Query<&mut Visibility, With<InterfaceNode>>,
    mut visibility_update_events: EventReader<NetworkData<messages::InterfaceVisibilityUpdate>>,
) {
    for visibility_updates in visibility_update_events.read() {
        for (interface_path, new_visibility) in visibility_updates.updates.iter() {
            let interface_entities = match interface_paths.get(interface_path) {
                Some(i) => i,
                None => {
                    net.disconnect(&format!(
                        "Server sent a visibility update for the interface node: '{}', but there is no node by that name.",
                        &interface_path
                    ));
                    return;
                }
            };

            for entity in interface_entities {
                let mut visibility = interface_query.get_mut(*entity).unwrap();

                if *new_visibility == 0 {
                    *visibility = Visibility::Inherited;
                } else if *new_visibility == 1 {
                    *visibility = Visibility::Hidden;
                } else if *new_visibility == 2 {
                    *visibility = Visibility::Visible;
                } else {
                    net.disconnect(&format!(
                        "Server sent an invalid visibility update for the interface: '{}'. The received visibility was ({}), but it must be one of:\
                        \n    0 = Visibility is inherited from the parent interface\
                        \n    1 = Hidden\
                        \n    2 = Visible",
                        &interface_path,
                        new_visibility
                        ));
                    return;
                }
            }
        }
    }
}

// This is a common hub for changing the visibility of interfaces. The client uses
// InterfaceToggleEvent, but the server will set visibility explicitly. Handlers are used to
// translate the server requests into toggle events by checking if the interface isn't already in
// the wanted state.
fn handle_toggle_events(
    mut keyboard_focus: ResMut<KeyboardFocus>,
    mut interface_stack: ResMut<InterfaceStack>,
    mut interface_query: Query<(Entity, &mut Visibility, &InterfaceConfig)>,
    mut interface_toggle_events: EventReader<InterfaceToggleEvent>,
) {
    for event in interface_toggle_events.read() {
        if *keyboard_focus == KeyboardFocus::Full {
            return;
        }

        let (_, mut visibility, toggled_config) =
            interface_query.get_mut(event.interface_entity).unwrap();
        if *visibility == Visibility::Visible {
            *keyboard_focus = KeyboardFocus::None;
            *visibility = Visibility::Hidden;
        } else {
            *keyboard_focus = toggled_config.keyboard_focus;
            *visibility = Visibility::Visible;
        }

        if toggled_config.is_exclusive {
            if *visibility == Visibility::Visible {
                for (entity, mut visibility, config) in interface_query.iter_mut() {
                    if entity != event.interface_entity && *visibility == Visibility::Visible {
                        *visibility = Visibility::Hidden;
                        if !config.is_exclusive {
                            interface_stack.push(entity);
                        }
                    }
                }
            } else if *visibility == Visibility::Hidden {
                for interface_entity in interface_stack.drain(..) {
                    let (_, mut visibility, _) = interface_query.get_mut(interface_entity).unwrap();
                    *visibility = Visibility::Visible;
                }
            }
        }
    }
}

fn handle_open_request(
    interfaces: Res<Interfaces>,
    net: Res<NetworkClient>,
    interface_query: Query<&Visibility, With<InterfaceConfig>>,
    mut interface_open_events: EventReader<NetworkData<messages::InterfaceOpen>>,
    mut interface_toggle_events: EventWriter<InterfaceToggleEvent>,
) {
    for event in interface_open_events.read() {
        let interface_entity = match interfaces.get(&event.interface_path) {
            Some(e) => e,
            None => {
                net.disconnect(&format!(
                    "Server sent open request for an interface with name: '{}', but there is no interface known by this name.",
                    event.interface_path
                ));
                return;
            }
        };
        if *interface_query.get(*interface_entity).unwrap() == Visibility::Hidden {
            interface_toggle_events.send(InterfaceToggleEvent {
                interface_entity: *interface_entity,
            });
        }
    }
}

fn handle_close_request(
    interfaces: Res<Interfaces>,
    net: Res<NetworkClient>,
    interface_query: Query<&Visibility, With<InterfaceConfig>>,
    mut interface_open_events: EventReader<NetworkData<messages::InterfaceClose>>,
    mut interface_toggle_events: EventWriter<InterfaceToggleEvent>,
) {
    for event in interface_open_events.read() {
        let interface_entity = match interfaces.get(&event.interface_path) {
            Some(e) => e,
            None => {
                net.disconnect(&format!(
                    "Server sent an interface with name '{}', but there is no interface known by this name.",
                    event.interface_path
                ));
                return;
            }
        };
        if *interface_query.get(*interface_entity).unwrap() == Visibility::Visible {
            interface_toggle_events.send(InterfaceToggleEvent {
                interface_entity: *interface_entity,
            });
        }
    }
}

fn handle_escape_key(
    mut keyboard_focus: ResMut<KeyboardFocus>,
    mut interface_stack: ResMut<InterfaceStack>,
    mut game_state: ResMut<NextState<GameState>>,
    mut interface_query: Query<(&mut Visibility, &InterfaceConfig)>,
    mut keyboard_input: EventReader<KeyboardInput>,
) {
    for key in keyboard_input.read() {
        if key.logical_key != Key::Escape || !key.state.is_pressed() {
            continue;
        }
        let mut was_open = false;
        for (mut visibility, config) in interface_query.iter_mut() {
            if *visibility == Visibility::Visible && config.is_exclusive {
                *visibility = Visibility::Hidden;
                *keyboard_focus = KeyboardFocus::None;
                was_open = true;
            }
        }

        if was_open {
            for interface_entity in interface_stack.drain(..) {
                let (mut visibility, _) = interface_query.get_mut(interface_entity).unwrap();
                *visibility = Visibility::Visible;
            }
        } else {
            game_state.set(GameState::Paused);
        }
    }
}
