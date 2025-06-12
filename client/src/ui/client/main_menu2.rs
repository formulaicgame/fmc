use std::{
    io::ErrorKind,
    net::{SocketAddr, ToSocketAddrs},
    path::{Path, PathBuf},
    time::Duration,
};

use bevy::{
    asset::load_internal_binary_asset,
    input::{
        keyboard::{Key, KeyboardInput},
        mouse::{MouseScrollUnit, MouseWheel},
        ButtonState,
    },
    prelude::*,
};
use serde::{Deserialize, Serialize};

use crate::ui::DEFAULT_FONT_HANDLE;

use super::GuiState;

const DOUBLE_CLICK_DELAY: f32 = 0.4;

const TAB_MAIN: Color = Color::srgb(31 as f32 / 255.0, 28 as f32 / 255.0, 25 as f32 / 255.0);
const TAB_SECONDARY: Color = Color::srgb(
    57.0 as f32 / 255.0,
    50.0 as f32 / 255.0,
    47.0 as f32 / 255.0,
);

const BASE_SIZE: Val = Val::Px(4.0);

pub struct MainMenuPlugin;
impl Plugin for MainMenuPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup).add_systems(
            Update,
            (
                switch_tab,
                handle_tab_clicks,
                scroll,
                update_textbox_text,
                search,
                edit_text_box,
                text_box_focus,
                handle_clicks,
            )
                .run_if(in_state(GuiState::MainMenu)),
        );
    }
}

fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn(Camera2d);

    commands
        .spawn((
            Interface,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: BASE_SIZE * 12.0,
                ..default()
            },
            ImageNode {
                image: asset_server.load("vista_blur.png"),
                ..default()
            },
            //BackgroundColor::from(Srgba::BLUE),
        ))
        .with_children(|parent| {
            // Header line
            parent
                .spawn((Node {
                    position_type: PositionType::Absolute,
                    flex_direction: FlexDirection::Column,
                    height: BASE_SIZE * 5.0,
                    width: Val::Percent(100.0),
                    ..default()
                },))
                .with_children(|parent| {
                    parent.spawn((
                        Node {
                            height: Val::Percent(50.0),
                            width: Val::Percent(100.0),
                            ..default()
                        },
                        BackgroundColor::from(Color::BLACK),
                    ));
                    parent.spawn((
                        Node {
                            height: Val::Percent(50.0),
                            width: Val::Percent(100.0),
                            ..default()
                        },
                        BackgroundColor::from(TAB_SECONDARY),
                    ));
                });

            // Tabs
            parent
                .spawn(Node {
                    width: Val::Percent(100.0),
                    height: Val::Percent(10.0),
                    justify_content: JustifyContent::SpaceEvenly,
                    ..default()
                })
                .with_children(|parent| {
                    parent.spawn_tab("Singleplayer", Tab::Worlds);
                    parent.spawn_tab("Multiplayer", Tab::Servers);
                });

            // World search bar / new world button
            parent
                .spawn((
                    Node {
                        width: Val::Percent(60.0),
                        height: Val::Percent(7.0),
                        column_gap: Val::Percent(3.0),
                        ..default()
                    },
                    Tab::Worlds,
                ))
                .with_children(|parent| {
                    parent
                        .spawn_textbox("Search/World name")
                        .insert(WorldTextBox);
                    parent
                        .spawn_button("New World", Srgba::GREEN, Some(Val::Percent(20.0)), None)
                        .insert(ButtonType::NewWorld);
                });

            // World list
            parent
                .spawn((
                    Node {
                        flex_direction: FlexDirection::Column,
                        //flex_grow: 1.0,
                        height: Val::Percent(83.0),
                        //padding: UiRect::top(Val::Percent(2.0)),
                        width: Val::Percent(60.0),
                        //justify_self: JustifySelf::Stretch,
                        overflow: Overflow::scroll(),
                        align_items: AlignItems::Center,
                        row_gap: BASE_SIZE * 4.0,
                        ..default()
                    },
                    WorldList,
                    Tab::Worlds,
                ))
                .with_children(|parent| {
                    for world in read_worlds() {
                        parent
                            .spawn_list_item(world.text(), &asset_server)
                            .insert(world);
                    }
                });
            // Worlds
            // parent
            //     .spawn((
            //         Node {
            //             width: Val::Percent(50.0),
            //             height: Val::Percent(90.0),
            //             flex_direction: FlexDirection::Column,
            //             align_items: AlignItems::Center,
            //             ..default()
            //         },
            //         Tab::Worlds,
            //     ))
            //     .with_children(|parent| {
            //         // Search bar / new world
            //     });

            // Servers
            parent
                .spawn((
                    Node {
                        display: Display::None,
                        width: Val::Percent(70.0),
                        height: Val::Percent(90.0),
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Center,
                        row_gap: Val::Px(20.0),
                        ..default()
                    },
                    Tab::Servers,
                ))
                .with_children(|parent| {
                    // Search bar / new world
                    parent
                        .spawn(Node {
                            width: Val::Percent(100.0),
                            height: Val::Percent(6.0),
                            column_gap: Val::Percent(3.0),
                            ..default()
                        })
                        .with_children(|parent| {
                            parent.spawn_textbox("Search/Address").insert(ServerTextBox);
                            parent
                                .spawn_button(
                                    "Connect",
                                    Srgba::GREEN,
                                    Some(Val::Percent(20.0)),
                                    None,
                                )
                                .insert(ButtonType::Connect);
                        });

                    // Server list
                    parent
                        .spawn((
                            Node {
                                flex_direction: FlexDirection::Column,
                                //padding: UiRect::top(Val::Percent(2.0)),
                                width: Val::Percent(100.0),
                                height: Val::Percent(94.0),
                                //justify_self: JustifySelf::Stretch,
                                overflow: Overflow::scroll(),
                                align_items: AlignItems::Center,
                                row_gap: Val::Percent(2.0),
                                ..default()
                            },
                            ServerList,
                        ))
                        .with_children(|parent| {
                            for server in read_servers() {
                                parent
                                    .spawn_list_item(&server.address, &asset_server)
                                    .insert(ListItem::Server(server.address));
                            }
                        });
                });

            // Footer line
            parent
                .spawn((Node {
                    position_type: PositionType::Absolute,
                    flex_direction: FlexDirection::Column,
                    bottom: Val::Percent(0.0),
                    height: BASE_SIZE * 5.0,
                    width: Val::Percent(100.0),
                    ..default()
                },))
                .with_children(|parent| {
                    parent.spawn((
                        Node {
                            height: Val::Percent(50.0),
                            width: Val::Percent(100.0),
                            ..default()
                        },
                        BackgroundColor::from(Srgba::rgba_u8(109, 99, 89, 255)),
                    ));
                    parent.spawn((
                        Node {
                            height: Val::Percent(50.0),
                            width: Val::Percent(100.0),
                            ..default()
                        },
                        BackgroundColor::from(Color::BLACK),
                    ));
                });
        });
}

trait MainMenuWidgets {
    fn spawn_tab<'a>(&'a mut self, text: &str, tab: Tab) -> EntityCommands<'a>;
    fn spawn_list_item<'a>(
        &'a mut self,
        name: &str,
        asset_server: &AssetServer,
    ) -> EntityCommands<'a>;
    fn spawn_textbox<'a>(&'a mut self, placeholder_text: &str) -> EntityCommands<'a>;
    fn spawn_button<'a>(
        &'a mut self,
        text: &str,
        color: Srgba,
        width: Option<Val>,
        height: Option<Val>,
    ) -> EntityCommands<'a>;
}

impl MainMenuWidgets for ChildBuilder<'_> {
    fn spawn_tab<'a>(&'a mut self, text: &str, tab: Tab) -> EntityCommands<'a> {
        let mut entity_commands = if tab == Tab::Worlds {
            self.spawn((
                tab,
                Node {
                    width: Val::Percent(30.0),
                    height: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                BackgroundColor::from(TAB_MAIN),
                Interaction::default(),
                Button,
            ))
        } else {
            self.spawn((
                tab,
                Node {
                    width: Val::Percent(30.0),
                    height: Val::Percent(90.0),
                    flex_direction: FlexDirection::Column,
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                BackgroundColor::from(Color::BLACK),
                Interaction::default(),
                Button,
            ))
        };
        entity_commands.with_children(|parent| {
            parent.spawn((
                Node {
                    bottom: Val::ZERO,
                    height: BASE_SIZE * 3.0,
                    width: Val::Percent(100.0),
                    position_type: PositionType::Absolute,
                    ..default()
                },
                BackgroundColor::from(TAB_SECONDARY),
            ));
            parent.spawn((
                Text::new(text),
                TextFont {
                    font: DEFAULT_FONT_HANDLE,
                    font_smoothing: bevy::text::FontSmoothing::AntiAliased,
                    ..default()
                },
            ));
        });

        return entity_commands;
    }

    fn spawn_list_item<'a>(
        &'a mut self,
        text: &str,
        asset_server: &AssetServer,
    ) -> EntityCommands<'a> {
        let mut entity_commands = self.spawn((
            Node {
                width: Val::Percent(100.0),
                height: BASE_SIZE * 27.0,
                flex_direction: FlexDirection::Column,
                ..default()
            },
            Interaction::default(),
            BackgroundColor::from(Color::srgb_u8(52, 52, 52)),
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
                                right: BASE_SIZE,
                                bottom: BASE_SIZE,
                                ..default()
                            },
                            ..default()
                        },
                        BorderColor::from(Srgba::rgb_u8(77, 77, 77)),
                    ));
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
                        BorderColor::from(Srgba::rgb_u8(100, 100, 100)),
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
                            BackgroundColor::from(Srgba::rgb_u8(58, 58, 58)),
                        ))
                        .with_children(|parent| {
                            // World preview image
                            parent.spawn((
                                Node {
                                    width: Val::Percent(15.0),
                                    height: Val::Percent(100.0),
                                    ..default()
                                },
                                ImageNode {
                                    image: asset_server.load("vista.png"),
                                    ..default()
                                },
                            ));
                            // Text container
                            parent
                                .spawn(Node {
                                    height: Val::Percent(100.0),
                                    margin: UiRect::left(Val::Percent(1.0)),
                                    flex_direction: FlexDirection::Column,
                                    justify_content: JustifyContent::SpaceEvenly,
                                    flex_grow: 1.0,
                                    ..default()
                                })
                                .with_children(|parent| {
                                    let font_size = 30.0;
                                    let text_font = TextFont {
                                        font: DEFAULT_FONT_HANDLE,
                                        font_size,
                                        font_smoothing: bevy::text::FontSmoothing::AntiAliased,
                                        ..default()
                                    };
                                    parent.spawn((
                                        Text::new(text),
                                        text_font.clone(),
                                        //shadow(font_size),
                                    ));
                                    parent.spawn((
                                        Text::new(text),
                                        text_font,
                                        // shadow(font_size)
                                    ));
                                });
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
                                    Interaction::default(),
                                    ButtonType::Edit(main_entity),
                                ))
                                .with_children(|parent| {
                                    parent.spawn((
                                        Node {
                                            width: BASE_SIZE * 8.0,
                                            height: BASE_SIZE * 8.0,
                                            ..default()
                                        },
                                        ImageNode {
                                            image: asset_server.load("edit.png"),
                                            ..default()
                                        },
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
                                    Interaction::default(),
                                    ButtonType::Play(main_entity),
                                ))
                                .with_children(|parent| {
                                    parent.spawn((
                                        Node {
                                            width: BASE_SIZE * 4.0,
                                            height: BASE_SIZE * 8.0,
                                            ..default()
                                        },
                                        ImageNode {
                                            image: asset_server.load("play.png"),
                                            ..default()
                                        },
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

    fn spawn_textbox<'a>(&'a mut self, placeholder_text: &str) -> EntityCommands<'a> {
        let mut entity_commands = self.spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                ..default()
            },
            Interaction::default(),
            BackgroundColor::from(Srgba::rgb_u8(53, 60, 74)),
        ));

        let mut text_entity = Entity::PLACEHOLDER;

        entity_commands.with_children(|parent| {
            parent
                .spawn(Node {
                    height: Val::Percent(100.0),
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
                        BorderColor::from(Srgba::rgb_u8(156, 156, 156)),
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
                        BorderColor::from(Srgba::rgb_u8(146, 146, 146)),
                    ));

                    let font_size = 20.0;
                    // Content
                    parent
                        .spawn(Node {
                            border: UiRect::all(BASE_SIZE),
                            height: Val::Percent(100.0),
                            width: Val::Percent(100.0),
                            padding: UiRect::left(Val::Percent(2.0)),
                            align_items: AlignItems::Center,
                            ..default()
                        })
                        .with_children(|parent| {
                            text_entity = parent
                                .spawn((
                                    Text::new("Search"),
                                    TextFont {
                                        font: DEFAULT_FONT_HANDLE.clone(),
                                        font_size,
                                        ..default()
                                    },
                                    //shadow(font_size),
                                ))
                                .id();
                        });
                });

            parent.spawn((
                Node {
                    height: BASE_SIZE * 2.0,
                    width: Val::Percent(100.0),
                    ..default()
                },
                BackgroundColor::from(Srgba::rgb_u8(52, 52, 52)),
            ));
        });

        entity_commands.insert(TextBox {
            placeholder: placeholder_text.to_owned(),
            text: String::new(),
            text_entity,
        });

        return entity_commands;
    }

    fn spawn_button<'a>(
        &'a mut self,
        text: &str,
        color: Srgba,
        width: Option<Val>,
        height: Option<Val>,
    ) -> EntityCommands<'a> {
        let mut border_color_one = color * 0.9;
        border_color_one.alpha = 1.0;
        let mut border_color_two = color * 0.8;
        border_color_two.alpha = 1.0;
        let mut shadow_color = color * 0.5;
        shadow_color.alpha = 1.0;
        let mut main_color = color * 0.7;
        main_color.alpha = 1.0;

        let mut entity_commands = self.spawn((
            Node {
                width: if let Some(width) = width {
                    width
                } else {
                    Val::Percent(100.0)
                },
                height: if let Some(height) = height {
                    height
                } else {
                    Val::Percent(100.0)
                },
                flex_direction: FlexDirection::Column,
                ..default()
            },
            Interaction::default(),
            // XXX: https://github.com/DioxusLabs/taffy/issues/834
            // This camouflages the error
            BackgroundColor::from(shadow_color),
        ));

        entity_commands.with_children(|parent| {
            parent
                .spawn(Node {
                    height: Val::Percent(100.0),
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
                        BorderColor::from(border_color_one),
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
                        BorderColor::from(border_color_two),
                    ));

                    // Content
                    parent
                        .spawn((
                            Node {
                                border: UiRect::all(BASE_SIZE),
                                height: Val::Percent(100.0),
                                width: Val::Percent(100.0),
                                overflow: Overflow::clip_x(),
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                ..default()
                            },
                            BackgroundColor::from(main_color),
                        ))
                        .with_children(|parent| {
                            let font_size = 24.0;
                            parent.spawn((
                                Text::new(text),
                                TextFont {
                                    font: DEFAULT_FONT_HANDLE.clone(),
                                    font_size,
                                    ..default()
                                },
                                //shadow(font_size),
                                TextLayout::new_with_no_wrap(),
                            ));
                        });
                });

            parent.spawn((
                Node {
                    height: BASE_SIZE * 2.0,
                    width: Val::Percent(100.0),
                    ..default()
                },
                BackgroundColor::from(shadow_color),
            ));
        });

        return entity_commands;
    }
}

#[derive(Component)]
struct WorldTextBox;

#[derive(Component)]
struct WorldList;

#[derive(Component)]
struct ServerTextBox;

#[derive(Component)]
struct ServerList;

#[derive(Component, PartialEq, Clone, Copy)]
enum Tab {
    Worlds,
    Servers,
}

#[derive(Component)]
struct Interface;

#[derive(Component)]
struct TextBox {
    text_entity: Entity,
    text: String,
    placeholder: String,
}

#[derive(Component)]
struct TextBoxText;

#[derive(Component)]
struct TextBoxFocus;

#[derive(Component)]
enum ButtonType {
    // Stores the entity of the list item it is part of.
    Play(Entity),
    Edit(Entity),
    Delete(Entity),
    NewWorld,
    Connect,
}

fn edit_text_box(
    mut focused_text_box: Query<&mut TextBox, With<TextBoxFocus>>,
    mut keyboard_input: EventReader<KeyboardInput>,
) {
    if let Ok(mut text_box) = focused_text_box.get_single_mut() {
        // TODO: There is currently no way to read the keyboard input properly. Res<Input<Keycode>> has
        // no utility function for discerning if it is a valid char, you have to match the whole thing,
        // but more importantly is does not consider the repeat properties of the WM.
        for input in keyboard_input.read() {
            if input.state != ButtonState::Pressed {
                continue;
            }

            match &input.logical_key {
                Key::Character(key) => {
                    text_box.text.push_str(key.as_str());
                }
                Key::Backspace => {
                    text_box.text.pop();
                }
                Key::Space => {
                    text_box.text.push(' ');
                }
                _ => (),
            }
        }
    }
}

fn handle_tab_clicks(mut tabs: Query<(&Interaction, &mut Node, &mut BackgroundColor), With<Tab>>) {
    let mut clicked = false;
    for (interaction, mut node, mut color) in tabs.iter_mut() {
        if *interaction == Interaction::Pressed {
            clicked = true;
            node.height = Val::Percent(100.0);
            *color = BackgroundColor::from(TAB_MAIN);
        }
    }

    if clicked {
        for (interaction, mut node, mut color) in tabs.iter_mut() {
            if *interaction != Interaction::Pressed {
                node.height = Val::Percent(90.0);
                *color = BackgroundColor::from(Color::BLACK);
            }
        }
    }
}

fn scroll(
    mut mouse_wheel: EventReader<MouseWheel>,
    tabs: Query<(&Node, &Tab), Without<Interaction>>,
    mut world_list: Query<&mut ScrollPosition, (With<WorldList>, Without<ServerList>)>,
    mut server_list: Query<&mut ScrollPosition, With<ServerList>>,
) {
    for mouse_wheel_event in mouse_wheel.read() {
        let mut open_tab = Tab::Worlds;
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

        let mut scroll_position = if open_tab == Tab::Worlds {
            world_list.get_single_mut().unwrap()
        } else {
            server_list.get_single_mut().unwrap()
        };

        scroll_position.offset_y -= dy;
    }
}

fn text_box_focus(
    mut commands: Commands,
    clicked_text_box: Query<(Entity, &Interaction), (With<TextBox>, Changed<Interaction>)>,
    previous_focus: Query<Entity, With<TextBoxFocus>>,
    mut keyboard_input: EventReader<KeyboardInput>,
) {
    let mut new_focus = false;

    for (entity, interaction) in clicked_text_box.iter() {
        if *interaction == Interaction::Pressed {
            commands.entity(entity).insert(TextBoxFocus);
            new_focus = true;
        }
    }

    for input in keyboard_input.read() {
        // Only trigger on first press
        if !input.state.is_pressed() {
            continue;
        }

        if input.logical_key == Key::Escape {
            new_focus = true;
        }
    }

    if new_focus {
        if let Ok(prev_entity) = previous_focus.get_single() {
            commands.entity(prev_entity).remove::<TextBoxFocus>();
        }
    }
}

fn update_textbox_text(
    time: Res<Time>,
    mut text_query: Query<&mut Text>,
    text_box_query: Query<(Ref<TextBox>, Has<TextBoxFocus>)>,
    mut removed_focus: RemovedComponents<TextBoxFocus>,
    mut cursor_timer: Local<Option<Timer>>,
    mut cursor_visible: Local<bool>,
) {
    for (text_box, has_focus) in text_box_query.iter() {
        if !text_box.is_changed() && !has_focus {
            continue;
        }

        let Ok(mut text) = text_query.get_mut(text_box.text_entity) else {
            continue;
        };
        text.clear();

        if !text_box.text.is_empty() {
            text.push_str(&text_box.text);
        } else if !has_focus {
            text.push_str(&text_box.placeholder);
        };

        if has_focus {
            let cursor_timer = cursor_timer.get_or_insert(Timer::new(
                Duration::from_secs_f32(0.5),
                TimerMode::Repeating,
            ));
            cursor_timer.tick(time.delta());

            if cursor_timer.just_finished() {
                *cursor_visible = !*cursor_visible;
            }

            if *cursor_visible {
                text.push('█');
            }
        }
    }

    for entity in removed_focus.read() {
        let Ok((text_box, _)) = text_box_query.get(entity) else {
            continue;
        };
        let Ok(mut text) = text_query.get_mut(text_box.text_entity) else {
            continue;
        };

        text.clear();

        if !text_box.text.is_empty() {
            text.push_str(&text_box.text);
        } else {
            text.push_str(&text_box.placeholder);
        };
    }
}

fn search(
    server_search_bar: Query<&TextBox, (Changed<TextBox>, With<TextBoxFocus>, With<ServerTextBox>)>,
    world_search_bar: Query<&TextBox, (Changed<TextBox>, With<TextBoxFocus>, With<WorldTextBox>)>,
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

                // Case-insensitive comparison
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

    if let Ok(textbox) = world_search_bar.get_single() {
        for (mut node, item) in worlds_and_servers.iter_mut() {
            if item.is_server() {
                continue;
            }

            if case_insensitive_search(item.text(), &textbox.text) {
                node.display = Display::Flex;
            } else {
                node.display = Display::None;
            }
        }
    };

    if let Ok(textbox) = server_search_bar.get_single() {
        for (mut node, item) in worlds_and_servers.iter_mut() {
            if item.is_world() {
                continue;
            }

            if case_insensitive_search(item.text(), &textbox.text) {
                node.display = Display::Flex;
            } else {
                node.display = Display::None;
            }
        }
    };
}

fn handle_clicks(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    list_items: Query<(Ref<Interaction>, &ListItem)>,
    world_input: Query<&TextBox, With<WorldTextBox>>,
    world_list: Query<Entity, With<WorldList>>,
    server_input: Query<&TextBox, With<ServerTextBox>>,
    button_clicks: Query<(&Interaction, &ButtonType), Changed<Interaction>>,
    mut last_click: Local<Option<std::time::Instant>>,
) {
    for (interaction, list_item) in list_items.iter() {
        if !interaction.is_changed() {
            continue;
        }

        let last_click = last_click.get_or_insert(std::time::Instant::now());

        if *interaction == Interaction::Pressed {
            if last_click.elapsed().as_secs_f32() < DOUBLE_CLICK_DELAY {
                dbg!("play");
            } else {
                *last_click = std::time::Instant::now();
            }
        }
    }

    for (interaction, button_type) in button_clicks.iter() {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match button_type {
            ButtonType::Play(list_item_entity) => {
                let (_, list_item) = list_items.get(*list_item_entity).unwrap();
                if list_item.is_server() {
                    dbg!("play server");
                } else if list_item.is_world() {
                    dbg!("play world");
                }
            }
            ButtonType::Edit(_) => {
                dbg!("edit");
            }
            ButtonType::Delete(_) => {
                dbg!("delete");
            }
            ButtonType::Connect => {
                let mut server_input = server_input.get_single().unwrap().text.clone();
                if !server_input.contains(":") {
                    server_input.push_str(":42069");
                }
                dbg!(server_input.to_socket_addrs());
            }
            ButtonType::NewWorld => {
                let world_input = world_input.get_single().unwrap().text.clone();
                let mut path = PathBuf::from("./local/share/fmc/worlds/");
                path.push(&world_input);

                let mut counter = 1;
                while path.exists() {
                    path.pop();
                    let new_name = world_input.clone() + " (" + &counter.to_string() + ")";
                    path.push(new_name);
                    counter += 1;
                }
                std::fs::create_dir(&path).ok();

                dbg!(&path);

                // HACK: In order to make the new list item appear at the top we need to
                // remove the child and add it again. There's currently no way
                // to specify the order of newly spawned children, they're always appended.
                // let mut new_item = Entity::PLACEHOLDER;
                // commands
                //     .entity(world_list.get_single().unwrap())
                //     .with_children(|parent| {
                //         let list_item = ListItem::World(path);
                //         new_item = parent
                //             .spawn_list_item(list_item.text(), &asset_server)
                //             .insert(list_item)
                //             .remove::<ChildOf>()
                //             .id();
                //     })
                //     .insert_children(0, &[new_item]);
            }
        }
    }
}

fn switch_tab(
    tab_buttons: Query<(&Interaction, &Tab), (Changed<Interaction>, With<Button>)>,
    mut tab_content: Query<(&mut Node, &Tab), Without<Button>>,
) {
    for (interaction, tab) in tab_buttons.iter() {
        if *interaction != Interaction::Pressed {
            continue;
        }

        for (mut node, tab_content) in tab_content.iter_mut() {
            if tab == tab_content {
                node.display = Display::Flex;
            } else {
                node.display = Display::None;
            }
        }
    }
}

// fn shadow(font_size: f32) -> TextShadow {
//     TextShadow {
//         offset: Vec2::splat(font_size / 12.0),
//         ..default()
//     }
// }

#[derive(Component)]
enum ListItem {
    Server(String),
    World(PathBuf),
}

impl ListItem {
    fn text(&self) -> &str {
        match self {
            Self::Server(address) => &address,
            Self::World(world) => world.file_name().unwrap().to_str().unwrap(),
        }
    }

    fn is_world(&self) -> bool {
        match self {
            Self::World(_) => true,
            _ => false,
        }
    }

    fn is_server(&self) -> bool {
        match self {
            Self::Server(_) => true,
            _ => false,
        }
    }
}

fn read_worlds() -> Vec<ListItem> {
    const PATH: &str = "./local/share/fmc/worlds";

    if let Err(e) = std::fs::create_dir_all(PATH) {
        error!("Could not create directory for worlds: {e}");
        return Vec::new();
    }
    let dir = match std::fs::read_dir(PATH) {
        Ok(d) => d,
        Err(e) => {
            error!("Could not read from worlds directory: {e}");
            return Vec::new();
        }
    };

    let mut result: Vec<(ListItem, std::time::SystemTime)> = Vec::new();
    for entry in dir {
        let Ok(entry) = entry else {
            continue;
        };
        // Must be a directory and the directory name must be valid utf-8
        if !entry.path().is_dir() || entry.path().file_name().map(|n| n.to_str()).is_none() {
            continue;
        }

        if let Ok(modified) = entry.metadata().and_then(|m| m.modified()) {
            result.push((ListItem::World(entry.path()), modified))
        } else {
            result.push((
                ListItem::World(entry.path()),
                std::time::SystemTime::UNIX_EPOCH,
            ));
        }
    }

    result.sort_by_key(|(_, t)| *t);
    result.reverse();
    result.into_iter().map(|(li, _)| li).collect()
}

#[derive(Serialize, Deserialize)]
struct ServerJson {
    favorite: bool,
    address: String,
}

fn read_servers() -> Vec<ServerJson> {
    let file = match std::fs::File::open(Path::new("~/.config/fmc/servers.json")) {
        Ok(f) => f,
        Err(e) if e.kind() != ErrorKind::NotFound => {
            error!("Failed to open servers.json: {e}");
            return Vec::new();
        }
        _ => return Vec::new(),
    };

    match serde_json::from_reader(file) {
        Ok(c) => c,
        Err(e) => {
            error!("Failed to read server list: {}", e);
            return Vec::new();
        }
    }
}
