use bevy::{
    color::palettes::css::DIM_GRAY,
    ecs::system::EntityCommands,
    input::{
        keyboard::{Key, KeyboardInput},
        ButtonState,
    },
    prelude::*,
    text::FontSmoothing,
};

use crate::ui::{text_input::TextBox, DEFAULT_FONT_HANDLE};

use super::BASE_SIZE;

const FONT_SIZE: f32 = 9.0;
const BORDER_SIZE: f32 = 1.0;

pub struct WidgetPlugin;
impl Plugin for WidgetPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, tint_button_on_hover);
    }
}

pub trait Widgets {
    /// A rectangular button with a centered label
    fn spawn_button<'a>(&'a mut self, label: &str, color: Srgba) -> EntityCommands<'a>;
    /// A rectangular textbox the user can input text into
    fn spawn_textbox<'a>(&'a mut self, placeholder_text: &str) -> EntityCommands<'a>;
    /// Spawns text with shadow
    fn spawn_text<'a>(&'a mut self, text: &str) -> EntityCommands<'a>;
}

// TODO: this function is const in 0.15
//const BUTTON_COLOR: Color = Color::srgb_u8(66, 66, 66);
const BUTTON_COLOR: Color = Color::srgb(0.26, 0.26, 0.26);

impl Widgets for ChildSpawnerCommands<'_> {
    fn spawn_button<'a>(&'a mut self, text: &str, color: Srgba) -> EntityCommands<'a> {
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
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
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

                    //let font_size = 20.0;
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
                                    Text::default(),
                                    TextFont {
                                        font: DEFAULT_FONT_HANDLE.clone(),
                                        font_size: FONT_SIZE,
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
            placeholder_text: placeholder_text.to_owned(),
            text: String::new(),
            text_entity,
        });

        return entity_commands;
        // let entity_commands = self.spawn((
        //     Button,
        //     Node {
        //         width: Val::Px(width),
        //         aspect_ratio: Some(width / 20.0),
        //         align_items: AlignItems::Center,
        //         justify_content: JustifyContent::Center,
        //         border: UiRect::all(Val::Px(BORDER_SIZE)),
        //         overflow: Overflow::clip(),
        //         ..default()
        //     },
        //     BackgroundColor::from(Color::srgb_u8(66, 66, 66)),
        //     BorderColor::from(Color::BLACK),
        //     TextBox::new(placeholder_text),
        // ));
        //
        // entity_commands
    }

    fn spawn_text<'a>(&'a mut self, text: &str) -> EntityCommands<'a> {
        self.spawn((
            Text::new(text),
            TextFont {
                font_size: FONT_SIZE,
                font: DEFAULT_FONT_HANDLE,
                font_smoothing: FontSmoothing::None,
                ..default()
            },
            TextColor(Color::WHITE),
            Node {
                position_type: PositionType::Absolute,
                ..default()
            },
            //TextShadow::default(),
        ))
    }
}

fn tint_button_on_hover(
    mut button_query: Query<
        (&Interaction, &mut BackgroundColor),
        (With<Button>, Changed<Interaction>),
    >,
) {
    for (interaction, mut background_color) in button_query.iter_mut() {
        match *interaction {
            Interaction::Hovered => {
                *background_color = (BUTTON_COLOR.to_srgba() * 1.25).into();
            }
            _ => {
                *background_color = BUTTON_COLOR.to_srgba().into();
            }
        }
    }
}

// #[derive(Component)]
// pub struct TextShadow {
//     shadow_entity: Entity,
// }
//
// impl Default for TextShadow {
//     fn default() -> Self {
//         Self {
//             shadow_entity: Entity::PLACEHOLDER,
//         }
//     }
// }

// fn add_text_shadow(
//     mut commands: Commands,
//     parent_style: Query<&Node>,
//     mut new_text_query: Query<
//         (&Parent, &Text, &TextFont, &TextLayout, &mut TextShadow),
//         Added<TextShadow>,
//     >,
// ) {
//     for (parent, text, font, layout, mut shadow) in new_text_query.iter_mut() {
//         // If an element is centered, the margin gets halved for some reason...
//         let parent_style = parent_style.get(parent.get()).unwrap();
//         let vertical_margin = match parent_style.flex_direction {
//             FlexDirection::Row | FlexDirection::RowReverse => {
//                 if parent_style.align_items == AlignItems::Center {
//                     Val::Px((font.font_size / 5.3).round())
//                 } else {
//                     Val::Px((font.font_size / 10.6).round())
//                 }
//             }
//             FlexDirection::Column | FlexDirection::ColumnReverse => {
//                 if parent_style.justify_content == JustifyContent::Center {
//                     Val::Px((font.font_size / 5.3).round())
//                 } else {
//                     Val::Px((font.font_size / 10.6).round())
//                 }
//             }
//         };
//
//         let horizontal_margin = match parent_style.flex_direction {
//             FlexDirection::Row | FlexDirection::RowReverse => {
//                 if parent_style.justify_content == JustifyContent::Center {
//                     Val::Px(font.font_size / 4.5)
//                 } else {
//                     Val::Px(font.font_size / 9.0)
//                 }
//             }
//             FlexDirection::Column | FlexDirection::ColumnReverse => {
//                 if parent_style.align_items == AlignItems::Center {
//                     Val::Px(font.font_size / 4.5)
//                 } else {
//                     Val::Px(font.font_size / 9.0)
//                 }
//             }
//         };
//
//         let shadow_text = commands.spawn((
//             text.clone(),
//             TextColor(DIM_GRAY.into()),
//             font.clone(),
//             layout.clone(),
//             Node {
//                 position_type: PositionType::Absolute,
//                 margin: UiRect {
//                     top: vertical_margin,
//                     left: horizontal_margin,
//                     ..default()
//                 },
//                 ..default()
//             },
//         ));
//
//         let entity = shadow_text.id();
//         shadow.shadow_entity = entity;
//         commands.entity(parent.get()).insert_children(0, &[entity]);
//     }
// }

// fn update_text_shadow(
//     text_query: Query<
//         (Ref<Text>, &Visibility, &TextShadow),
//         Or<(Changed<Visibility>, Changed<Text>)>,
//     >,
//     mut shadow_text_query: Query<(&mut Text, &mut Visibility), Without<TextShadow>>,
// ) {
//     for (text, visibility, shadow) in text_query.iter() {
//         if text.is_added() {
//             continue;
//         }
//
//         let Ok((mut shadow_text, mut shadow_visibility)) =
//             shadow_text_query.get_mut(shadow.shadow_entity)
//         else {
//             continue;
//         };
//
//         *shadow_text = text.clone();
//         *shadow_visibility = *visibility;
//     }
// }
