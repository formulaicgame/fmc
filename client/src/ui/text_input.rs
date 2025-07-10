use std::time::Duration;

use bevy::{
    input::{
        keyboard::{Key, KeyboardInput},
        ButtonState,
    },
    prelude::*,
};

use super::{DEFAULT_FONT_HANDLE, DEFAULT_FONT_SIZE};

pub(super) struct TextInputPlugin;
impl Plugin for TextInputPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (add_text, focus, (edit, update_text).chain()));
    }
}

/// A text input field. A text component will be inserted as a child of the entity unless a text
/// entity is explicitly provided.
#[derive(Component)]
pub struct TextBox {
    pub width: Val,
    pub height: Val,
    pub text_entity: Entity,
    pub text: String,
    pub placeholder_text: String,
    pub autofocus: bool,
}

impl TextBox {
    pub fn new(placeholder_text: &str) -> Self {
        Self {
            placeholder_text: placeholder_text.to_owned(),
            ..default()
        }
    }

    pub fn with_autofocus(mut self) -> Self {
        self.autofocus = true;
        self
    }

    pub fn with_text(mut self, text: &str) -> Self {
        self.text = text.to_owned();
        self
    }
}

impl Default for TextBox {
    fn default() -> Self {
        Self {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            text_entity: Entity::PLACEHOLDER,
            text: String::new(),
            placeholder_text: String::new(),
            autofocus: false,
        }
    }
}

#[derive(Component)]
pub struct TextBoxFocus;

fn focus(
    mut commands: Commands,
    clicked_text_box: Query<
        (Entity, &Interaction),
        (With<TextBox>, Changed<Interaction>, Without<TextBoxFocus>),
    >,
    newly_visible: Query<(Entity, &TextBox, &InheritedVisibility), Changed<InheritedVisibility>>,
    previous_focus: Query<(Entity, &InheritedVisibility), With<TextBoxFocus>>,
    mut keyboard_input: EventReader<KeyboardInput>,
) {
    let mut new_focus = false;

    for (entity, interaction) in clicked_text_box.iter() {
        if *interaction == Interaction::Pressed {
            commands.entity(entity).insert(TextBoxFocus);
            new_focus = true;
            break;
        }
    }

    for (entity, text_box, visibility) in newly_visible.iter() {
        if text_box.autofocus && visibility.get() {
            commands.entity(entity).insert(TextBoxFocus);
            new_focus = true;
            break;
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

    if let Ok((entity, visibility)) = previous_focus.single() {
        if new_focus || !visibility.get() {
            commands.entity(entity).remove::<TextBoxFocus>();
        }
    }
}

// TODO: Do this as hook? No need to keep it running, it only happens on gui setup and server
// interface setup
fn add_text(
    mut commands: Commands,
    mut added_text_boxes: Query<(Entity, &mut TextBox), Added<TextBox>>,
) {
    for (entity, mut text_box) in added_text_boxes.iter_mut() {
        if text_box.text_entity != Entity::PLACEHOLDER {
            continue;
        }

        commands.entity(entity).with_children(|parent| {
            text_box.text_entity = parent
                .spawn((
                    Text::new(&text_box.text),
                    TextFont {
                        font_size: DEFAULT_FONT_SIZE,
                        font: DEFAULT_FONT_HANDLE,
                        ..default()
                    },
                    TextColor(Color::WHITE),
                    Node {
                        position_type: PositionType::Absolute,
                        ..default()
                    },
                    TextShadow {
                        offset: Vec2::splat(DEFAULT_FONT_SIZE / 12.0),
                        ..default()
                    },
                ))
                .id();
        });
    }
}

// TODO: TextBox should have an autofocus field.
// fn focus_text_box_on_interface_change(
//     mut commands: Commands,
//     focused_text_box: Query<Entity, With<FocusedTextBox>>,
//     text_box_query: Query<
//         (Entity, &InheritedVisibility),
//         (With<TextBox>, Changed<InheritedVisibility>),
//     >,
// ) {
//     for (entity, visibility) in text_box_query.iter() {
//         if let Ok(prev_entity) = focused_text_box.get_single() {
//             commands.entity(prev_entity).remove::<FocusedTextBox>();
//         }
//
//         if visibility.get() {
//             commands.entity(entity).insert(FocusedTextBox);
//             return;
//         }
//     }
// }

fn edit(
    mut focused_text_box: Query<&mut TextBox, With<TextBoxFocus>>,
    mut keyboard_input: EventReader<KeyboardInput>,
) {
    if let Ok(mut text_box) = focused_text_box.single_mut() {
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

fn update_text(
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
            text.push_str(&text_box.placeholder_text);
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
            text.push_str(&text_box.placeholder_text);
        };
    }
}
