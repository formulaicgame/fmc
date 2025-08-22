use std::collections::HashMap;

use bevy::{
    ecs::system::EntityCommands,
    image::{CompressedImageFormats, ImageSampler},
    prelude::*,
    render::render_asset::RenderAssetUsages,
};
use fmc_protocol::messages;
use serde::Deserialize;

use crate::{
    game_state::GameState,
    networking::NetworkClient,
    ui::{text_input::TextBox, DEFAULT_FONT_HANDLE, DEFAULT_FONT_SIZE},
};

use self::items::{CursorItemBox, ItemBoxSection};

use super::{CursorVisibility, UiState};

pub mod items;
pub mod key_bindings;
mod text;

const INTERFACE_CONFIG_PATH: &str = "server_assets/active/interfaces/";
const INTERFACE_TEXTURE_PATH: &str = "server_assets/active/textures/interfaces/";

pub struct ServerInterfacesPlugin;
impl Plugin for ServerInterfacesPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<InterfaceVisibilityEvent>()
            .insert_resource(InterfaceStack::default())
            .add_plugins((
                items::ItemPlugin,
                text::TextPlugin,
                key_bindings::KeyBindingsPlugin,
            ))
            .add_systems(
                Update,
                (
                    button_interaction.run_if(in_state(UiState::ServerInterfaces)),
                    handle_server_node_visibility_updates,
                    handle_server_interface_visibility_updates,
                    handle_visibility_events,
                )
                    .run_if(in_state(GameState::Playing)),
            )
            .add_systems(
                Update,
                interface_visibility.run_if(state_changed::<UiState>),
            )
            .add_systems(OnEnter(GameState::Launcher), cleanup);
    }
}

// This is inserted for every node in the interface that has an interface path. For easy reverse
// lookup when updates are sent to the server.
#[derive(Component)]
pub struct InterfaceNode {
    pub path: String,
}

// Many interfaces may share the same interface paths. This is so that the same information can be
// displayed in different interfaces. You might for example want items shown in an inventory, to
// also be shown in a hotbar, an update will then reflect in both.
#[derive(Resource, Deref, DerefMut, Default)]
pub struct InterfacePaths(HashMap<String, Vec<Entity>>);

// Maps interface names to their root entity, e.g. "inventory", but NOT "inventory/equipment".
// The interface names are extracted from their file names.
#[derive(Resource, Deref, DerefMut, Default)]
pub struct Interfaces(HashMap<String, Entity>);

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
            net.disconnect(format!(
                "Misconfigured assets: Failed to read from interface configuration directory at '{}'\n\
                Error: {}",
                INTERFACE_CONFIG_PATH, e
            ));
            return;
        }
    };

    for dir_entry in directory {
        let entry = match dir_entry {
            Ok(d) => d,
            Err(e) => {
                net.disconnect(format!(
                    "Failed to read entry in the interface configuration directory\nError: {}",
                    e
                ));
                return;
            }
        };

        // Skip non-game interfaces, they have their own directory
        if entry.file_type().is_ok_and(|t| t.is_dir()) {
            continue;
        }

        let file_path = entry.path();

        let file = match std::fs::File::open(&file_path) {
            Ok(f) => f,
            Err(e) => {
                net.disconnect(format!(
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
                net.disconnect(format!(
                    "Misconfigured assets: Failed to read interface configuration at: '{}'\n\
                Error: {}",
                    &file_path.display(),
                    e
                ));
                return;
            }
        };

        // NOTE(WORKAROUND): When spawning an Image, the dimensions of the image are
        // inferred, but if it has children they're discarded and it uses the size of the children
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
                bevy::image::ImageType::Extension("png"),
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

            let node: Node = if let Some(image_path) = &config.image {
                let dimensions = read_image_dimensions(&image_path);
                let mut node: Node = config.style.clone().into();
                node.width = Val::Px(dimensions.x);
                node.height = Val::Px(dimensions.y);
                node
            } else {
                config.style.clone().into()
            };

            entity_commands.insert((
                node,
                BackgroundColor::from(config.background_color.unwrap_or(Color::NONE)),
                BorderColor::from(config.border_color.unwrap_or(Color::NONE)),
                Interaction::default(),
            ));

            if let Some(path) = &config.image {
                entity_commands.insert(ImageNode {
                    image: asset_server.load(INTERFACE_TEXTURE_PATH.to_owned() + &path),
                    ..default()
                });
            }

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
                NodeContent::TextContainer {
                    text_background_color,
                    fade,
                } => {
                    entity_commands.insert(text::TextContainer {
                        text_background_color: text_background_color.unwrap_or(Color::NONE),
                    });

                    if *fade {
                        entity_commands.insert(text::FadeLines);
                    }
                }
                NodeContent::TextBox => {
                    entity_commands.insert(TextBox::default().with_autofocus());
                }
                NodeContent::Text {
                    text,
                    font_size,
                    color,
                } => {
                    entity_commands.with_children(|parent| {
                        parent.spawn((
                            Node {
                                position_type: PositionType::Absolute,
                                ..default()
                            },
                            Text::new(text),
                            TextFont {
                                font_size: *font_size,
                                font: DEFAULT_FONT_HANDLE,
                                ..default()
                            },
                            TextColor(*color),
                            TextShadow {
                                offset: Vec2::splat(DEFAULT_FONT_SIZE / 12.0),
                                ..default()
                            },
                        ));
                    });
                }
                NodeContent::None => (),
            }
        }

        let interface_entity = commands
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                InterfaceConfig {
                    is_exclusive: node_config.exclusive,
                    keyboard_focus: node_config.keyboard_focus,
                },
                Visibility::Hidden,
            ))
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
            .id();

        // (Probably) safe to unwrap here, as it has already loaded a file with the name.
        let interface_name = file_path.file_stem().unwrap().to_string_lossy().to_string();

        interfaces.insert(interface_name, interface_entity);
    }

    commands.insert_resource(interface_paths);
    commands.insert_resource(interfaces);

    commands
        .spawn((
            ImageNode::default(),
            Node {
                width: Val::Px(14.0),
                height: Val::Px(15.8),
                position_type: PositionType::Absolute,
                flex_direction: FlexDirection::ColumnReverse,
                align_items: AlignItems::FlexEnd,
                ..default()
            },
            ZIndex(1),
            CursorItemBox::default(),
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::default(),
                Node {
                    top: Val::Px(1.0),
                    left: Val::Px(2.0),
                    ..default()
                },
            ));
        });
}

fn cleanup(
    mut commands: Commands,
    cursor_item_box: Query<Entity, With<CursorItemBox>>,
    interfaces: Option<Res<Interfaces>>,
    mut interface_stack: ResMut<InterfaceStack>,
) {
    if let Ok(entity) = cursor_item_box.single() {
        commands.entity(entity).despawn();
    }

    interface_stack.clear();
    if let Some(interfaces) = interfaces {
        for interface_entity in interfaces.values() {
            commands.entity(*interface_entity).despawn();
        }
    }
}

/// Event used by keybindings to toggle an interface open or closed.
#[derive(Event)]
struct InterfaceVisibilityEvent {
    pub interface_entity: Entity,
    pub visible: bool,
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

// TODO: This is not fully implemented. Should allow you to move around when some interfaces are open
#[derive(Deserialize, Default, PartialEq, Clone, Copy, Debug)]
enum KeyboardFocus {
    // Keyboard focus is not taken
    #[default]
    None,
    // Takes away movement, other keys work
    Movement,
    // Interface consumes all keyboard input, only Escape will close it.
    Full,
}

// Wrapper for 'Style' that is deserializable
#[derive(Deserialize, Clone, Debug)]
#[serde(default, deny_unknown_fields)]
struct NodeStyle {
    display: Display,
    box_sizing: BoxSizing,
    position_type: PositionType,
    overflow: Overflow,
    overflow_clip_margin: OverflowClipMargin,
    left: Val,
    right: Val,
    top: Val,
    bottom: Val,
    width: Val,
    height: Val,
    min_width: Val,
    min_height: Val,
    max_width: Val,
    max_height: Val,
    aspect_ratio: Option<f32>,
    align_items: AlignItems,
    justify_items: JustifyItems,
    align_self: AlignSelf,
    justify_self: JustifySelf,
    align_content: AlignContent,
    justify_content: JustifyContent,
    margin: Rect,
    padding: Rect,
    border: Rect,
    flex_direction: FlexDirection,
    flex_wrap: FlexWrap,
    flex_grow: f32,
    flex_shrink: f32,
    flex_basis: Val,
    row_gap: Val,
    column_gap: Val,
    grid_auto_flow: GridAutoFlow,
    grid_template_rows: Vec<RepeatedGridTrack>,
    grid_template_columns: Vec<RepeatedGridTrack>,
    grid_auto_rows: Vec<GridTrack>,
    grid_auto_columns: Vec<GridTrack>,
    grid_row: GridPlacement,
    grid_column: GridPlacement,
}

impl Default for NodeStyle {
    fn default() -> Self {
        Self {
            display: Display::DEFAULT,
            box_sizing: BoxSizing::DEFAULT,
            position_type: PositionType::DEFAULT,
            left: Val::Auto,
            right: Val::Auto,
            top: Val::Auto,
            bottom: Val::Auto,
            flex_direction: FlexDirection::DEFAULT,
            flex_wrap: FlexWrap::DEFAULT,
            align_items: AlignItems::DEFAULT,
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
            overflow_clip_margin: OverflowClipMargin::DEFAULT,
            row_gap: Val::ZERO,
            column_gap: Val::ZERO,
            grid_auto_flow: GridAutoFlow::DEFAULT,
            grid_template_rows: Vec::new(),
            grid_template_columns: Vec::new(),
            grid_auto_rows: Vec::new(),
            grid_auto_columns: Vec::new(),
            grid_column: GridPlacement::DEFAULT,
            grid_row: GridPlacement::DEFAULT,
        }
    }
}

impl From<NodeStyle> for Node {
    fn from(value: NodeStyle) -> Self {
        Node {
            display: value.display,
            box_sizing: value.box_sizing,
            position_type: value.position_type,
            overflow: value.overflow,
            overflow_clip_margin: value.overflow_clip_margin,
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
    TextContainer {
        // TODO: Maybe this should be part of the InterfaceTextBoxUpdate message instead, so that
        // you can have individual colors for each line.
        //
        // If you do not want the container itself to have color, this allows you to set the
        // background color of the lines themselves.
        text_background_color: Option<Color>,
        // If true, new lines will be set to visible when received and then faded out after a short
        // interval.
        #[serde(default)]
        fade: bool,
    },
    // Text input
    TextBox,
    // A text field
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

#[derive(Resource, Deref, DerefMut, Default)]
struct InterfaceStack(Vec<Entity>);

// TODO: Interaction isn't granular enough. Buttons should only be pressed when they are hovered
// over and the button is released. https://github.com/bevyengine/bevy/pull/9240 is fix I think.
// Remember to also change it for the client GUI buttons, it will solve the problem
// of mouse button spillover. Currently it plays the item use animation when you come out of the
// pause interface.
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

fn handle_server_node_visibility_updates(
    net: Res<NetworkClient>,
    interface_paths: Res<InterfacePaths>,
    mut interface_query: Query<&mut Visibility, With<InterfaceNode>>,
    mut visibility_update_events: EventReader<messages::InterfaceNodeVisibilityUpdate>,
) {
    for visibility_updates in visibility_update_events.read() {
        for (interface_path, should_be_visible) in visibility_updates.updates.iter() {
            let interface_entities = match interface_paths.get(interface_path) {
                Some(i) => i,
                None => {
                    net.disconnect(&format!(
                        "Server sent a visibility update for the interface node: '{}', but there is no node with that name.",
                        &interface_path
                    ));
                    return;
                }
            };

            for interface_entity in interface_entities.iter().cloned() {
                let mut visibility = interface_query.get_mut(interface_entity).unwrap();

                if *visibility == Visibility::Hidden && *should_be_visible {
                    *visibility = Visibility::Inherited;
                } else if *visibility == Visibility::Inherited && !should_be_visible {
                    *visibility = Visibility::Hidden;
                }
            }
        }
    }
}

fn handle_server_interface_visibility_updates(
    interfaces: Res<Interfaces>,
    net: Res<NetworkClient>,
    interface_query: Query<&Visibility, With<InterfaceConfig>>,
    mut server_interface_visibility_events: EventReader<messages::InterfaceVisibilityUpdate>,
    mut interface_visibility_events: EventWriter<InterfaceVisibilityEvent>,
) {
    for event in server_interface_visibility_events.read() {
        let interface_entity = match interfaces.get(&event.interface_path) {
            Some(e) => e,
            None => {
                net.disconnect(&format!(
                    "Server sent open request for an interface with name: '{}', but there is no interface with that name.",
                    event.interface_path
                ));
                return;
            }
        };

        interface_visibility_events.write(InterfaceVisibilityEvent {
            interface_entity: *interface_entity,
            visible: event.visible,
        });
    }
}

fn handle_visibility_events(
    ui_state: Res<State<UiState>>,
    mut cursor_visibility: ResMut<CursorVisibility>,
    mut interface_stack: ResMut<InterfaceStack>,
    mut interface_query: Query<(Entity, &mut Visibility, &InterfaceConfig)>,
    mut interface_visibility_events: EventReader<InterfaceVisibilityEvent>,
) {
    for event in interface_visibility_events.read() {
        if *ui_state.get() == UiState::Gui {
            // Store any events received from the server while the gui is open
            if event.visible {
                interface_stack.push(event.interface_entity);
            } else {
                interface_stack.retain(|e| *e != event.interface_entity);
            }
            continue;
        }

        let (_, mut visibility, interface_config) =
            interface_query.get_mut(event.interface_entity).unwrap();

        if event.visible {
            *visibility = Visibility::Inherited;
        } else {
            *visibility = Visibility::Hidden;
        }

        if interface_config.is_exclusive {
            if *visibility == Visibility::Inherited {
                cursor_visibility.server = true;

                for (entity, mut visibility, config) in interface_query.iter_mut() {
                    if entity != event.interface_entity && *visibility == Visibility::Inherited {
                        *visibility = Visibility::Hidden;
                        if !config.is_exclusive {
                            interface_stack.push(entity);
                        }
                    }
                }
            } else if *visibility == Visibility::Hidden {
                cursor_visibility.server = false;

                for interface_entity in interface_stack.drain(..) {
                    let (_, mut visibility, _) = interface_query.get_mut(interface_entity).unwrap();
                    *visibility = Visibility::Inherited;
                }
            }
        }
    }
}

fn interface_visibility(
    ui_state: Res<State<UiState>>,
    mut interface_stack: ResMut<InterfaceStack>,
    mut interface_toggle_events: EventWriter<InterfaceVisibilityEvent>,
    mut interface_query: Query<(Entity, &mut Visibility, &InterfaceConfig)>,
) {
    if *ui_state.get() == UiState::Gui {
        for (interface_entity, mut visibility, _) in interface_query.iter_mut() {
            if *visibility == Visibility::Inherited {
                *visibility = Visibility::Hidden;
                interface_stack.push(interface_entity);
            }
        }
    } else if *ui_state.get() == UiState::ServerInterfaces {
        while let Some(interface_entity) = interface_stack.pop() {
            let (_, _, interface_config) = interface_query.get(interface_entity).unwrap();
            interface_toggle_events.write(InterfaceVisibilityEvent {
                interface_entity,
                visible: true,
            });

            if interface_config.is_exclusive {
                return;
            }
        }
    }
}
