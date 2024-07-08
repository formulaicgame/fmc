use std::collections::HashMap;

use bevy::{app::AppExit, prelude::*};
use fmc_networking::{messages, NetworkClient};
use serde::Deserialize;

use crate::{
    game_state::GameState,
    ui::server::{InterfaceToggleEvent, Interfaces},
};

pub struct KeyBindingsPlugin;
impl Plugin for KeyBindingsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                handle_key_presses.run_if(in_state(GameState::Playing)),
                //escape_key.run_if(in_state(GameState::Playing)),
            ),
        );
    }
}

// TODO: Key combos?
#[derive(Resource, Deref, DerefMut, Default)]
struct KeyBindings {
    inner: HashMap<KeyCode, String>,
}

#[derive(Deserialize)]
struct KeyBindingJson {
    command: String,
    key_binding: String,
}

// TODO: Parse commands into some format that can be readily executed.
//       Commands that open/close interfaces should not have to parse the string AND check that it
//       is a valid command.
pub fn load_key_bindings(mut commands: Commands, net: Res<NetworkClient>) {
    let path = "./server_assets/commands.json";
    let file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(e) => {
            net.disconnect(format!(
                "Misconfigured resource pack: Failed to read commands file at '{}'\nError: {}",
                path, e
            ));
            return;
        }
    };
    let bindings_json: Vec<KeyBindingJson> = match serde_json::from_reader(file) {
        Ok(c) => c,
        Err(e) => {
            net.disconnect(format!(
                "Misconfigured resource pack: Failed to read commands file at '{}'\nError: {}",
                path, e
            ));
            return;
        }
    };

    let mut key_bindings = KeyBindings::default();
    for binding in bindings_json.into_iter() {
        let keycode = match binding.key_binding.as_str() {
            "a" => KeyCode::KeyA,
            "b" => KeyCode::KeyB,
            "c" => KeyCode::KeyC,
            "d" => KeyCode::KeyD,
            "e" => KeyCode::KeyE,
            "f" => KeyCode::KeyF,
            "g" => KeyCode::KeyG,
            "h" => KeyCode::KeyH,
            "i" => KeyCode::KeyI,
            "j" => KeyCode::KeyJ,
            "k" => KeyCode::KeyK,
            "l" => KeyCode::KeyL,
            "m" => KeyCode::KeyM,
            "n" => KeyCode::KeyN,
            "o" => KeyCode::KeyO,
            "p" => KeyCode::KeyP,
            "q" => KeyCode::KeyQ,
            "r" => KeyCode::KeyR,
            "s" => KeyCode::KeyS,
            "t" => KeyCode::KeyT,
            "u" => KeyCode::KeyU,
            "v" => KeyCode::KeyV,
            "w" => KeyCode::KeyW,
            "x" => KeyCode::KeyX,
            "y" => KeyCode::KeyY,
            "z" => KeyCode::KeyZ,
            key => {
                net.disconnect(format!(
                    "Misconfigured resource pack: Failed to map key to command. Can't map key: '{}', only a-z is allowed",
                    key
                ));
                continue;
            }
        };

        key_bindings.insert(keycode, binding.command);
    }

    commands.insert_resource(key_bindings);
}

//fn handle_key_presses(
//    net: Res<NetworkClient>,
//    input: Res<Input<KeyCode>>,
//    key_bindings: Res<KeyBindings>,
//    interface_entities: Res<Interfaces>,
//    mut interface_query: Query<(Entity, &mut Style), With<Interface>>,
//) {
//    for pressed_key in input.get_just_pressed() {
//        if let Some(command) = key_bindings.get(pressed_key) {
//            if let Some(interface_name) = command.strip_prefix("/interface ") {
//                // TODO: This should be pre parsed so it fails on connection
//                let entity = match interface_entities.get(interface_name) {
//                    Some(e) => e,
//                    None => {
//                        net.disconnect(&format!(
//                            "Misconfigured resource pack: Improperly configured keybindings, \
//                                command: '{}', mapped to '{:?}' could not be parsed.",
//                            &command, pressed_key
//                        ));
//                        return;
//                    }
//                };
//
//                // Only one interface can be visible. Since the root node has Position::Relative.
//                // Otherwise they mess with each other's layout.
//                for (e, mut style) in interface_query.iter_mut() {
//                    if *entity != e {
//                        style.display = Display::None
//                    }
//                }
//
//                let (_, mut style) = interface_query.get_mut(*entity).unwrap();
//                match style.display {
//                    Display::Flex => style.display = Display::None,
//                    Display::None => style.display = Display::Flex,
//                }
//
//                //interface_query.get_mut(*entity).unwrap().is_visible ^= true;
//            } else {
//                net.send_message(messages::ChatMessage {
//                    username: "".to_owned(),
//                    message: command.to_owned(),
//                })
//            }
//        }
//    }
//}

// TODO: Pre-parse key bindings to make sure they are valid. This way we fail at connection, and
// can drop validation when using them.
fn handle_key_presses(
    net: Res<NetworkClient>,
    input: Res<ButtonInput<KeyCode>>,
    key_bindings: Res<KeyBindings>,
    interfaces: Res<Interfaces>,
    mut interface_events: EventWriter<InterfaceToggleEvent>,
) {
    for pressed_key in input.get_just_pressed() {
        if let Some(command) = key_bindings.get(pressed_key) {
            if let Some(interface_name) = command.strip_prefix("/interface ") {
                let entity = match interfaces.get(interface_name) {
                    Some(e) => e,
                    None => {
                        net.disconnect(&format!(
                            "Misconfigured resource pack: Improperly configured keybindings, \
                                command: '{}', mapped to '{:?}' could not be parsed.",
                            &command, pressed_key
                        ));
                        return;
                    }
                };

                interface_events.send(InterfaceToggleEvent {
                    interface_entity: *entity,
                });
            } else {
                net.send_message(messages::InterfaceTextInput {
                    interface_path: "key".to_owned(),
                    text: command.clone(),
                });
            }
        }
    }
}

fn escape_key(
    net: Res<NetworkClient>,
    mut exit_events: EventWriter<AppExit>,
    input: Res<ButtonInput<KeyCode>>,
) {
    if input.just_pressed(KeyCode::Escape) {
        net.disconnect("");
        exit_events.send(AppExit);
    }
}
