use bevy::{ecs::system::EntityCommands, prelude::*};

use super::DEFAULT_FONT_HANDLE;

const FONT_SIZE: f32 = 9.0;

pub struct WidgetPlugin;
impl Plugin for WidgetPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                text_input_setup,
                edit_text_box,
                update_textbox_text.after(edit_text_box),
                focus_text_box_on_click,
                focus_text_box_on_interface_change,
                tint_button_on_hover,
            ),
        );
    }
}

const BORDER_SIZE: f32 = 1.0;

pub trait Widgets {
    /// The default GUI button.  
    fn spawn_button<'a>(&'a mut self, width: f32, text: &str) -> EntityCommands<'a>;
    /// The default GUI textbox
    fn spawn_textbox<'a>(&'a mut self, width: f32, text: &str) -> EntityCommands<'a>;
    fn spawn_text(
        &mut self,
        text: &str,
        font_size: f32,
        color: Color,
        flex_direction: FlexDirection,
        justify_content: JustifyContent,
        align_items: AlignItems,
    );
}

impl Widgets for ChildBuilder<'_> {
    fn spawn_button<'a>(&'a mut self, width: f32, text: &str) -> EntityCommands<'a> {
        let mut entity_commands = self.spawn((ButtonBundle {
            background_color: Color::rgb_u8(110, 110, 110).into(),
            border_color: Color::BLACK.into(),
            style: Style {
                aspect_ratio: Some(width / 20.0),
                width: Val::Px(width),
                border: UiRect::all(Val::Px(BORDER_SIZE)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            ..default()
        },));
        entity_commands.with_children(|parent| {
            parent
                // Need to spawn a parent here because the borders mess up, expanding into the
                // parent border when their position type is Absolute.
                .spawn(NodeBundle {
                    style: Style {
                        width: Val::Percent(100.0),
                        height: Val::Percent(100.0),
                        ..default()
                    },
                    ..default()
                })
                .with_children(|parent| {
                    parent.spawn((NodeBundle {
                        style: Style {
                            position_type: PositionType::Absolute,
                            width: Val::Percent(100.0),
                            height: Val::Percent(100.0),
                            border: UiRect {
                                top: Val::Px(BORDER_SIZE),
                                left: Val::Px(BORDER_SIZE),
                                ..default()
                            },
                            ..default()
                        },
                        border_color: Color::rgb_u8(170, 170, 170).into(),
                        ..default()
                    },));
                    parent.spawn((NodeBundle {
                        style: Style {
                            position_type: PositionType::Absolute,
                            width: Val::Percent(100.0),
                            height: Val::Percent(100.0),
                            border: UiRect {
                                bottom: Val::Px(BORDER_SIZE),
                                right: Val::Px(BORDER_SIZE),
                                ..default()
                            },
                            ..default()
                        },
                        border_color: Color::rgba_u8(62, 62, 62, 150).into(),
                        ..default()
                    },));
                });
            parent.spawn_text(
                text,
                FONT_SIZE,
                Color::WHITE,
                FlexDirection::Row,
                JustifyContent::Center,
                AlignItems::Center,
            );
        });
        entity_commands
    }

    fn spawn_textbox<'a>(&'a mut self, width: f32, text: &str) -> EntityCommands<'a> {
        let entity_commands = self.spawn((
            ButtonBundle {
                background_color: Color::BLACK.into(),
                border_color: Color::WHITE.into(),
                style: Style {
                    width: Val::Percent(width),
                    aspect_ratio: Some(width / 4.2),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    border: UiRect::all(Val::Px(BORDER_SIZE)),
                    overflow: Overflow::clip(),
                    ..default()
                },
                ..default()
            },
            TextBox {
                is_input: true,
                text: text.to_owned(),
                scrollable: false,
                scroll_position: 0.0,
                text_background_color: Color::NONE,
            },
            TextInput::default(),
        ));

        entity_commands
    }

    // TODO: https://github.com/bevyengine/bevy/pull/8973 related pr that would remove need to fake
    // shadows.
    // TODO: None of this alignment stuff should be part of the function, but shadows need to be
    // shifted twice as far when centered. I don't know why. Waiting for
    // 0.12 to see if Cosmic text fixes it.
    fn spawn_text(
        &mut self,
        text: &str,
        font_size: f32,
        color: Color,
        flex_direction: FlexDirection,
        justify_content: JustifyContent,
        align_items: AlignItems,
    ) {
        let vertical_margin = match flex_direction {
            FlexDirection::Row | FlexDirection::RowReverse => {
                if align_items == AlignItems::Center {
                    Val::Px((font_size / 5.3).round())
                } else {
                    Val::Px((font_size / 10.6).round())
                }
            }
            FlexDirection::Column | FlexDirection::ColumnReverse => {
                if justify_content == JustifyContent::Center {
                    Val::Px((font_size / 5.3).round())
                } else {
                    Val::Px((font_size / 10.6).round())
                }
            }
        };

        let horizontal_margin = match flex_direction {
            FlexDirection::Row | FlexDirection::RowReverse => {
                if justify_content == JustifyContent::Center {
                    Val::Px(font_size / 4.5)
                } else {
                    Val::Px(font_size / 9.0)
                }
            }
            FlexDirection::Column | FlexDirection::ColumnReverse => {
                if align_items == AlignItems::Center {
                    Val::Px(font_size / 4.5)
                } else {
                    Val::Px(font_size / 9.0)
                }
            }
        };

        self.spawn((
            TextBundle {
                text: Text::from_section(
                    text,
                    TextStyle {
                        font_size,
                        font: DEFAULT_FONT_HANDLE,
                        color: Color::DARK_GRAY,
                        ..default()
                    },
                ),
                style: Style {
                    position_type: PositionType::Absolute,
                    margin: UiRect {
                        top: vertical_margin,
                        left: horizontal_margin,
                        ..default()
                    },
                    ..default()
                },
                ..default()
            },
            TextMarker,
        ));
        self.spawn((
            TextBundle {
                text: Text::from_section(
                    text,
                    TextStyle {
                        font_size,
                        font: DEFAULT_FONT_HANDLE,
                        color,
                        ..default()
                    },
                ),
                style: Style {
                    position_type: PositionType::Absolute,
                    ..default()
                },
                ..default()
            },
            TextMarker,
        ));
    }
}

#[derive(Component, Deref, Default)]
struct PreviousButtonColor(Color);

fn tint_button_on_hover(
    mut commands: Commands,
    new_button_query: Query<Entity, Added<Button>>,
    mut button_query: Query<
        (&Interaction, &mut PreviousButtonColor, &mut BackgroundColor),
        (With<Button>, Changed<Interaction>),
    >,
) {
    for entity in new_button_query.iter() {
        commands
            .entity(entity)
            .insert(PreviousButtonColor::default());
    }

    for (interaction, mut prev_color, mut background_color) in button_query.iter_mut() {
        match *interaction {
            Interaction::Hovered => {
                prev_color.0 = background_color.0;
                background_color.0 *= Vec3::splat(139.0 / 110.0);
            }
            Interaction::None => {
                background_color.0 = prev_color.0;
            }
            _ => (),
        }
    }
}

#[derive(Component)]
pub struct FocusedTextBox;

// By the GUI this is used exlcusively as text input.
// By the server interfaces, it is also used as a text container the server can place text into.
#[derive(Component, Default)]
pub struct TextBox {
    pub is_input: bool,
    // If this is an input textbox, this is the entire content of the input field. The visible text
    // might be a subset of this.
    // XXX: Will not be updated unless it is an input textbox.
    pub text: String,
    pub scrollable: bool,
    pub scroll_position: f32,
    pub text_background_color: Color,
}

#[derive(Component, Default)]
struct TextInput {
    cursor: usize,
}

#[derive(Component)]
struct TextMarker;

fn text_input_setup(
    mut commands: Commands,
    input_query: Query<(Entity, &TextBox, &Style), Added<TextBox>>,
) {
    for (entity, text_box, style) in input_query.iter() {
        if !text_box.is_input {
            continue;
        }

        commands
            .entity(entity)
            .insert(TextInput::default())
            .with_children(|parent| {
                parent.spawn_text(
                    &text_box.text,
                    FONT_SIZE,
                    Color::WHITE,
                    style.flex_direction,
                    style.justify_content,
                    style.align_items,
                );
            });
    }
}

fn focus_text_box_on_click(
    mut commands: Commands,
    focused_text_box: Query<Entity, With<FocusedTextBox>>,
    possible_new_focus: Query<(Entity, &Interaction), (With<TextInput>, Changed<Interaction>)>,
) {
    for (entity, interaction) in possible_new_focus.iter() {
        if *interaction == Interaction::Pressed {
            if let Ok(prev_entity) = focused_text_box.get_single() {
                commands.entity(prev_entity).remove::<FocusedTextBox>();
            }

            commands.entity(entity).insert(FocusedTextBox);
        }
    }
}

fn focus_text_box_on_interface_change(
    mut commands: Commands,
    focused_text_box: Query<Entity, With<FocusedTextBox>>,
    text_box_query: Query<
        (Entity, &InheritedVisibility),
        (With<TextInput>, Changed<InheritedVisibility>),
    >,
) {
    for (entity, visibility) in text_box_query.iter() {
        if let Ok(prev_entity) = focused_text_box.get_single() {
            commands.entity(prev_entity).remove::<FocusedTextBox>();
        }

        if visibility.get() {
            commands.entity(entity).insert(FocusedTextBox);
            return;
        }
    }
}

fn edit_text_box(
    mut focused_text_box: Query<&mut TextBox, With<FocusedTextBox>>,
    mut chars: EventReader<ReceivedCharacter>,
) {
    if let Ok(mut text_box) = focused_text_box.get_single_mut() {
        // TODO: There is currently no way to read the keyboard input properly. Res<Input<Keycode>> has
        // no utility function for discerning if it is a valid char, you have to match the whole thing,
        // but more importantly is does not consider the repeat properties of the WM.
        for event in chars.read() {
            let char = event.char.chars().last().unwrap();
            if char.is_ascii() {
                if !char.is_control() {
                    text_box.text.push(char);
                } else if char == '\u{8}' {
                    // This is backspace (pray)
                    text_box.text.pop();
                }
            }
        }
    }
}

fn update_textbox_text(
    mut text_query: Query<&mut Text, With<TextMarker>>,
    text_box_query: Query<(&TextBox, &Children), Changed<TextBox>>,
) {
    for (text_box, children) in text_box_query.iter() {
        for child in children {
            if let Ok(mut text) = text_query.get_mut(*child) {
                text.sections[0].value = text_box.text.clone();
            }
        }
    }
}
