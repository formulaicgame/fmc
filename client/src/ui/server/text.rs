use bevy::{prelude::*, text::LineBreak};
use fmc_protocol::messages;

use crate::{
    game_state::GameState,
    networking::NetworkClient,
    ui::{
        text_input::{TextBox, TextBoxFocus},
        DEFAULT_FONT_HANDLE,
    },
};

use super::{InterfaceNode, InterfacePaths};

pub struct TextPlugin;
impl Plugin for TextPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                handle_text_updates,
                //change_line_size,
                send_text,
                fade_lines,
            )
                .run_if(in_state(GameState::Playing)),
        );
    }
}

#[derive(Component)]
pub struct TextContainer {
    pub text_background_color: Color,
}

#[derive(Component)]
struct Line;

#[derive(Component)]
struct Fade {
    delay: Timer,
}

// Marks text containers that should have their new lines faded out.
#[derive(Component)]
pub struct FadeLines;

fn handle_text_updates(
    mut commands: Commands,
    net: Res<NetworkClient>,
    interface_paths: Res<InterfacePaths>,
    text_container_query: Query<(Option<&Children>, &TextContainer, Has<FadeLines>)>,
    mut text_update_events: EventReader<messages::InterfaceTextUpdate>,
) {
    for text_update in text_update_events.read() {
        let interface_entities = match interface_paths.get(&text_update.interface_path) {
            Some(i) => i,
            None => {
                net.disconnect(&format!(
                    "Server sent text update for interface with name: {}, but there is no interface by that name.",
                    &text_update.interface_path
                ));
                return;
            }
        };

        for interface_entity in interface_entities.iter() {
            let (children, text_container, should_fade) = match text_container_query
                .get(*interface_entity)
            {
                Ok(c) => c,
                Err(_) => {
                    net.disconnect(&format!(
                            "Server sent text update for interface with name: {}, but the interface is not configured to contain text.",
                            &text_update.interface_path
                            ));
                    return;
                }
            };

            let mut entity_commands = if children.is_none() {
                let entity = commands.spawn(Text::default()).id();
                commands.entity(*interface_entity).add_child(entity);
                commands.entity(entity)
            } else if let Some(child_entity) = children.unwrap().get(text_update.index as usize) {
                let mut e = commands.entity(*child_entity);
                e.despawn_descendants();
                e
            } else {
                let entity = commands.spawn(Text::default()).id();

                if text_update.index < 0 {
                    commands.entity(*interface_entity).add_children(&[entity]);
                } else {
                    commands
                        .entity(*interface_entity)
                        .insert_children(0, &[entity]);
                }

                commands.entity(entity)
            };

            entity_commands.insert((
                Node {
                    // XXX: Since the fake shadow text extends a little farther it
                    // often wraps before the real text. To counteract this the real
                    // text is made to wrap a little sooner by shrinking the width.
                    width: Val::Percent(98.0),
                    ..default()
                },
                BackgroundColor::from(text_container.text_background_color),
                Line,
            ));

            let color: Color = match Srgba::hex(&text_update.color) {
                Ok(c) => c.into(),
                Err(_) => {
                    net.disconnect(&format!(
                                    "Server sent malformed text box update for interface with name: {}. The text contained a malformed color property. '{}', is not a valid hex color.",
                                    &text_update.interface_path,
                                    &text_update.color
                                    ));
                    return;
                }
            };
            entity_commands.with_child((
                Text::new(&text_update.text),
                TextColor::from(color),
                TextFont {
                    font: DEFAULT_FONT_HANDLE,
                    font_size: text_update.font_size,
                    ..default()
                },
                TextLayout {
                    linebreak: LineBreak::WordOrCharacter,
                    justify: JustifyText::Left,
                },
                //TextShadow::default(),
            ));

            if should_fade {
                entity_commands.insert((
                    Fade {
                        delay: Timer::from_seconds(10.0, TimerMode::Once),
                    },
                    Visibility::Visible,
                ));
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

// Increase background size vertically to make room for fake shadow text
// fn change_line_size(
//     mut line_query: Query<(&mut Node, &Children), Added<Line>>,
//     layout_query: Query<&TextLayoutInfo>,
// ) {
//     for (mut node, children) in line_query.iter_mut() {
//         let layout = layout_query.get(children[0]).unwrap();
//         node.height = Val::Px(layout.logical_size.y + 1.0);
//     }
// }

fn send_text(
    net: Res<NetworkClient>,
    mut focused_text_box: Query<(&mut TextBox, &InterfaceNode), With<TextBoxFocus>>,
    keyboard: Res<ButtonInput<KeyCode>>,
) {
    if !keyboard.just_pressed(KeyCode::Enter) {
        return;
    }

    if let Ok((mut text_box, interface_node)) = focused_text_box.get_single_mut() {
        if text_box.text.is_empty() {
            return;
        }

        net.send_message(messages::InterfaceTextInput {
            interface_path: interface_node.path.clone(),
            text: text_box.text.clone(),
        });
        text_box.text.clear();
    }
}
