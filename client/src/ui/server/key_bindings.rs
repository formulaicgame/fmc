use std::collections::HashMap;

use bevy::prelude::*;
use fmc_protocol::messages;
use serde::Deserialize;

use crate::{
    networking::NetworkClient,
    ui::{
        client::GuiState,
        server::{InterfaceVisibilityEvent, Interfaces},
        UiState,
    },
};

use super::{InterfaceConfig, KeyboardFocus};

pub struct KeyBindingsPlugin;
impl Plugin for KeyBindingsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            handle_key_presses.run_if(in_state(UiState::ServerInterfaces)),
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
    let path = "./server_assets/active/commands.json";
    let file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(e) => {
            net.disconnect(format!(
                "Misconfigured assets: Failed to read commands file at '{}'\nError: {}",
                path, e
            ));
            return;
        }
    };
    let bindings_json: Vec<KeyBindingJson> = match serde_json::from_reader(file) {
        Ok(c) => c,
        Err(e) => {
            net.disconnect(format!(
                "Misconfigured assets: Failed to read commands file at '{}'\nError: {}",
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
                    "Misconfigured assets: Failed to map key to command. Can't map key: '{}', only a-z is allowed",
                    key
                ));
                continue;
            }
        };

        key_bindings.insert(keycode, binding.command);
    }

    commands.insert_resource(key_bindings);
}

// TODO: Pre-parse key bindings to make sure they are valid. This way we fail at connection, and
// can drop validation when using them.
fn handle_key_presses(
    net: Res<NetworkClient>,
    input: Res<ButtonInput<KeyCode>>,
    key_bindings: Res<KeyBindings>,
    interfaces: Res<Interfaces>,
    mut gui_state: ResMut<NextState<GuiState>>,
    interface_query: Query<(Entity, &Visibility, &InterfaceConfig)>,
    mut interface_events: EventWriter<InterfaceVisibilityEvent>,
) {
    for pressed_key in input.get_just_pressed() {
        // Any open interface can be closed by pressing "e" or "escape". "e" will only close it if
        // the interface doesn't take keyboard focus.
        for (interface_entity, visibility, interface_config) in interface_query.iter() {
            if visibility != Visibility::Hidden && interface_config.is_exclusive {
                if (*pressed_key == KeyCode::KeyE
                    && interface_config.keyboard_focus != KeyboardFocus::Full)
                    || *pressed_key == KeyCode::Escape
                {
                    interface_events.send(InterfaceVisibilityEvent {
                        interface_entity,
                        visible: false,
                    });
                    return;
                } else if interface_config.keyboard_focus == KeyboardFocus::Full {
                    // If the keyboard is taken, input should be ignored unless it is to close it.
                    return;
                }
            }
        }

        // TODO: This isn't sufficient, the server can spam open interfaces, don't know how to handle it
        if *pressed_key == KeyCode::Escape {
            gui_state.set(GuiState::PauseMenu);
        }

        if let Some(command) = key_bindings.get(pressed_key) {
            if let Some(interface_name) = command.strip_prefix("/interface ") {
                let entity = match interfaces.get(interface_name) {
                    Some(e) => e,
                    None => {
                        net.disconnect(&format!(
                            "Misconfigured assets: Improperly configured keybindings, \
                                command: '{}', mapped to '{:?}' could not be parsed.",
                            &command, pressed_key
                        ));
                        return;
                    }
                };

                interface_events.send(InterfaceVisibilityEvent {
                    interface_entity: *entity,
                    visible: true,
                });
            } else {
                // If it's not an interface, the server handles it.
                net.send_message(messages::InterfaceTextInput {
                    interface_path: "key".to_owned(),
                    text: command.clone(),
                });
            }
        }
    }
}
