use bevy::{prelude::*, text::TextLayoutInfo};
use fmc_networking::{messages, NetworkClient, NetworkData};

use crate::{
    game_state::GameState,
    ui::{
        widgets::{FocusedTextBox, TextBox},
        DEFAULT_FONT_HANDLE,
    },
};

use super::{InterfaceNode, InterfacePaths};

pub struct TextBoxPlugin;
impl Plugin for TextBoxPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                size_textbox_lines,
                handle_text_box_updates,
                send_text,
                fade_lines,
            )
                .run_if(GameState::in_game),
        );
    }
}

#[derive(Component)]
struct Line;

#[derive(Component)]
struct Fade {
    delay: Timer,
}

// Marks textboxes that should have their new lines faded out.
#[derive(Component)]
pub struct FadeLines;

// TODO: Why can't I drain these events? I don't want to re-allocate the strings. I remember last
// time I tried I had to do DerefMut on NetworkData, but now that doesn't work anymore. Does it
// have to do with the internal types of the event itself?
fn handle_text_box_updates(
    mut commands: Commands,
    net: Res<NetworkClient>,
    interface_paths: Res<InterfacePaths>,
    text_box_query: Query<(Option<&Children>, &TextBox, Has<FadeLines>)>,
    mut text_box_update_events: EventReader<NetworkData<messages::InterfaceTextBoxUpdate>>,
) {
    for text_box_update in text_box_update_events.read() {
        let interface_entities = match interface_paths.get(&text_box_update.interface_path) {
            Some(i) => i,
            None => {
                net.disconnect(&format!(
                    "Server sent item box update for interface with name: {}, but there is no interface by that name.",
                    &text_box_update.interface_path
                ));
                return;
            }
        };

        for interface_entity in interface_entities.iter() {
            let (children, text_box, should_fade) = match text_box_query.get(*interface_entity) {
                Ok(c) => c,
                Err(_) => {
                    net.disconnect(&format!(
                            "Server sent text box update for interface with name: {}, but the interface is not configured to contain text.",
                            &text_box_update.interface_path
                            ));
                    return;
                }
            };

            for new_line in &text_box_update.lines {
                let mut sections = Vec::with_capacity(new_line.sections.len());
                let mut shadow_sections = Vec::with_capacity(new_line.sections.len());
                for section in &new_line.sections {
                    let color = match Color::hex(&section.color) {
                        Ok(c) => c,
                        Err(_) => {
                            net.disconnect(&format!(
                                    "Server malformed text box update for interface with name: {}. The text contained a malformed color property. '{}', is not a valid hex color.",
                                    &text_box_update.interface_path,
                                    &section.color
                                    ));
                            return;
                        }
                    };
                    sections.push(TextSection {
                        value: section.text.clone(),
                        style: TextStyle {
                            font: DEFAULT_FONT_HANDLE,
                            font_size: section.font_size,
                            color,
                        },
                    });
                    shadow_sections.push(TextSection {
                        value: section.text.clone(),
                        style: TextStyle {
                            font: DEFAULT_FONT_HANDLE,
                            font_size: section.font_size,
                            color: Color::DARK_GRAY,
                        },
                    });
                }

                let mut entity_commands = if children.is_none() {
                    let entity = commands.spawn_empty().id();
                    commands.entity(*interface_entity).add_child(entity);
                    commands.entity(entity)
                } else if let Some(child_entity) = children.unwrap().get(new_line.index as usize) {
                    let mut e = commands.entity(*child_entity);
                    e.despawn_descendants();
                    e
                } else {
                    let entity = commands.spawn_empty().id();

                    if new_line.index < 0 {
                        commands.entity(*interface_entity).push_children(&[entity]);
                    } else {
                        commands
                            .entity(*interface_entity)
                            .insert_children(0, &[entity]);
                    }

                    commands.entity(entity)
                };

                if should_fade {
                    entity_commands.insert(Fade {
                        delay: Timer::from_seconds(10.0, TimerMode::Once),
                    });
                }

                // TODO: Move font size to the line instead of in the secions.
                // XXX: This relies on all sections being the same font size
                let height = sections[0].style.font_size;

                entity_commands.insert((
                    NodeBundle {
                        style: Style {
                            width: Val::Percent(100.0),
                            ..default()
                        },
                        visibility: if should_fade {
                            Visibility::Visible
                        } else {
                            Visibility::Inherited
                        },
                        background_color: text_box.text_background_color.into(),
                        ..default()
                    },
                    Line,
                ));

                entity_commands.with_children(|parent| {
                    parent.spawn(TextBundle {
                        text: Text::from_sections(shadow_sections),
                        style: Style {
                            width: Val::Percent(100.0),
                            position_type: PositionType::Absolute,
                            margin: UiRect {
                                top: Val::Px(height / 10.6),
                                left: Val::Px(height / 9.0),
                                ..default()
                            },
                            ..default()
                        },
                        ..default()
                    });
                    parent.spawn(TextBundle {
                        text: Text::from_sections(sections),
                        style: Style {
                            width: Val::Percent(100.0),
                            position_type: PositionType::Absolute,
                            ..default()
                        },
                        ..default()
                    });
                });
            }
        }
    }
}

// TODO: Make it actually fade? Thinking maybe it should initially show text without background,
// this way it's less obtrusive.
fn fade_lines(
    mut commands: Commands,
    time: Res<Time>,
    mut fading_query: Query<(Entity, &mut Visibility, &mut Fade)>,
) {
    for (entity, mut visibility, mut fade) in fading_query.iter_mut() {
        if !fade.delay.finished() {
            fade.delay.tick(time.delta());
        } else {
            commands.entity(entity).remove::<Fade>();
            *visibility = Visibility::Inherited;
        }
    }
}

fn size_textbox_lines(
    mut line_query: Query<(&mut Style, &Children), Added<Line>>,
    layout_query: Query<&TextLayoutInfo>,
) {
    for (mut style, children) in line_query.iter_mut() {
        let layout = layout_query.get(children[0]).unwrap();
        style.height = Val::Px(layout.logical_size.y + 1.0);
    }
}

fn send_text(
    net: Res<NetworkClient>,
    mut focused_text_box: Query<(&mut TextBox, &InterfaceNode), With<FocusedTextBox>>,
    keyboard: Res<ButtonInput<KeyCode>>,
) {
    if !keyboard.just_pressed(KeyCode::Enter) {
        return;
    }

    if let Ok((mut text_box, interface_node)) = focused_text_box.get_single_mut() {
        net.send_message(messages::InterfaceTextInput {
            interface_path: interface_node.path.clone(),
            text: text_box.text.clone(),
        });
        text_box.text.clear();
    }
}
