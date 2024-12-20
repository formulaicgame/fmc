use bevy::{color::palettes::css::DARK_GRAY, ecs::system::EntityCommands, prelude::*};

use super::DEFAULT_FONT_HANDLE;

const FONT_SIZE: f32 = 9.0;
const BORDER_SIZE: f32 = 1.0;

pub struct WidgetPlugin;
impl Plugin for WidgetPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                textbox_setup,
                edit_text_box,
                update_textbox_text.after(edit_text_box),
                focus_text_box_on_click,
                focus_text_box_on_interface_change,
                add_text_shadow,
                update_text_shadow,
                tint_button_on_hover,
            ),
        );
    }
}

pub trait Widgets {
    /// A rectangular button with a centered label
    fn spawn_button<'a>(&'a mut self, width: f32, label: &str) -> EntityCommands<'a>;
    /// A rectangular textbox the user can input text into
    fn spawn_textbox<'a>(&'a mut self, width: f32, placeholder_text: &str) -> EntityCommands<'a>;
    /// Spawns text with shadow, the text can be changed by querying for ShadowText
    fn spawn_text<'a>(&'a mut self, text: &str) -> EntityCommands<'a>;
}

// TODO: this function is const in 0.15
//const BUTTON_COLOR: Color = Color::srgb_u8(66, 66, 66);
const BUTTON_COLOR: Color = Color::srgb(0.26, 0.26, 0.26);

impl Widgets for ChildBuilder<'_> {
    fn spawn_button<'a>(&'a mut self, width: f32, text: &str) -> EntityCommands<'a> {
        let mut entity_commands = self.spawn(ButtonBundle {
            background_color: BUTTON_COLOR.into(),
            style: Style {
                aspect_ratio: Some(width / 20.0),
                width: Val::Px(width),
                border: UiRect::all(Val::Px(BORDER_SIZE)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            border_color: Color::BLACK.into(),
            ..default()
        });
        entity_commands.with_children(|parent| {
            parent
                .spawn(NodeBundle {
                    style: Style {
                        width: Val::Percent(100.0),
                        height: Val::Percent(100.0),
                        border: UiRect::all(Val::Px(BORDER_SIZE)),
                        ..default()
                    },
                    border_color: Color::srgb_u8(128, 128, 128).into(),
                    ..default()
                })
                .with_children(|first_border| {
                    first_border.spawn(NodeBundle {
                        style: Style {
                            width: Val::Percent(100.0),
                            height: Val::Percent(100.0),
                            border: UiRect::all(Val::Px(BORDER_SIZE)),
                            ..default()
                        },
                        border_color: Color::BLACK.into(),
                        ..default()
                    });
                });
            parent.spawn_text(text);
        });
        entity_commands
    }

    fn spawn_textbox<'a>(&'a mut self, width: f32, placeholder_text: &str) -> EntityCommands<'a> {
        let entity_commands = self.spawn((
            ButtonBundle {
                background_color: Color::srgb_u8(66, 66, 66).into(),
                border_color: Color::BLACK.into(),
                style: Style {
                    width: Val::Px(width),
                    aspect_ratio: Some(width / 20.0),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    border: UiRect::all(Val::Px(BORDER_SIZE)),
                    overflow: Overflow::clip(),
                    ..default()
                },
                ..default()
            },
            TextBox {
                text: placeholder_text.to_owned(),
            },
        ));

        entity_commands
    }

    fn spawn_text<'a>(&'a mut self, text: &str) -> EntityCommands<'a> {
        self.spawn((
            TextBundle {
                text: Text::from_section(
                    text,
                    TextStyle {
                        font_size: FONT_SIZE,
                        font: DEFAULT_FONT_HANDLE,
                        color: Color::WHITE,
                        ..default()
                    },
                ),
                style: Style {
                    position_type: PositionType::Absolute,
                    ..default()
                },
                ..default()
            },
            TextShadow::default(),
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

///
#[derive(Component, Default)]
pub struct TextBox {
    pub text: String,
}

#[derive(Component)]
pub struct FocusedTextBox;

#[derive(Component)]
struct TextBoxText;

fn textbox_setup(mut commands: Commands, input_query: Query<(Entity, &TextBox), Added<TextBox>>) {
    for (entity, text_box) in input_query.iter() {
        commands.entity(entity).with_children(|parent| {
            parent.spawn_text(&text_box.text).insert(TextBoxText);
        });
    }
}

fn focus_text_box_on_click(
    mut commands: Commands,
    focused_text_box: Query<Entity, With<FocusedTextBox>>,
    possible_new_focus: Query<(Entity, &Interaction), (With<TextBox>, Changed<Interaction>)>,
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
        (With<TextBox>, Changed<InheritedVisibility>),
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
    mut text_query: Query<&mut Text>,
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

#[derive(Component)]
pub struct TextShadow {
    shadow_entity: Entity,
}

impl Default for TextShadow {
    fn default() -> Self {
        Self {
            shadow_entity: Entity::PLACEHOLDER,
        }
    }
}

fn add_text_shadow(
    mut commands: Commands,
    parent_style: Query<&Style>,
    mut text_query: Query<(&Parent, &Text, &mut TextShadow), Added<TextShadow>>,
) {
    for (parent, text, mut shadow) in text_query.iter_mut() {
        // If an element is centered, the margin gets halved for some reason...
        let font_size = text.sections[0].style.font_size;
        let parent_style = parent_style.get(parent.get()).unwrap();
        let vertical_margin = match parent_style.flex_direction {
            FlexDirection::Row | FlexDirection::RowReverse => {
                if parent_style.align_items == AlignItems::Center {
                    Val::Px((font_size / 5.3).round())
                } else {
                    Val::Px((font_size / 10.6).round())
                }
            }
            FlexDirection::Column | FlexDirection::ColumnReverse => {
                if parent_style.justify_content == JustifyContent::Center {
                    Val::Px((font_size / 5.3).round())
                } else {
                    Val::Px((font_size / 10.6).round())
                }
            }
        };

        let horizontal_margin = match parent_style.flex_direction {
            FlexDirection::Row | FlexDirection::RowReverse => {
                if parent_style.justify_content == JustifyContent::Center {
                    Val::Px(font_size / 4.5)
                } else {
                    Val::Px(font_size / 9.0)
                }
            }
            FlexDirection::Column | FlexDirection::ColumnReverse => {
                if parent_style.align_items == AlignItems::Center {
                    Val::Px(font_size / 4.5)
                } else {
                    Val::Px(font_size / 9.0)
                }
            }
        };
        let mut shadow_text = text.clone();
        shadow_text
            .sections
            .iter_mut()
            .for_each(|section| section.style.color = DARK_GRAY.into());
        let shadow_text_entity = commands
            .spawn(TextBundle {
                text: shadow_text,
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
            })
            .id();

        shadow.shadow_entity = shadow_text_entity;
        commands
            .entity(parent.get())
            .insert_children(0, &[shadow_text_entity]);
    }
}

fn update_text_shadow(
    text_query: Query<(Ref<Text>, &TextShadow)>,
    mut shadow_text_query: Query<&mut Text, Without<TextShadow>>,
) {
    for (text, shadow) in text_query.iter() {
        if text.is_added() || !text.is_changed() {
            continue;
        }

        let mut new_shadow_text = text.clone();
        new_shadow_text
            .sections
            .iter_mut()
            .for_each(|section| section.style.color = DARK_GRAY.into());
        let mut shadow_text = shadow_text_query.get_mut(shadow.shadow_entity).unwrap();
        *shadow_text = new_shadow_text;
    }
}
