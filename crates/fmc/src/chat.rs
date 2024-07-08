use bevy::prelude::*;
use fmc_networking::{messages, NetworkData, NetworkServer, ServerNetworkEvent, Username};

use crate::players::Player;

pub const CHAT_FONT_SIZE: f32 = 8.0;
pub const CHAT_TEXT_COLOR: &str = "#ffffff";

pub struct ChatPlugin;
impl Plugin for ChatPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (handle_chat_messages, send_connection_messages));
    }
}

fn handle_chat_messages(
    net: Res<NetworkServer>,
    player_query: Query<&Username>,
    mut chat_message_query: EventReader<NetworkData<messages::InterfaceTextInput>>,
) {
    for chat_message in chat_message_query.read() {
        if &chat_message.interface_path != "chat/input" {
            continue;
        }
        let Ok(username) = player_query.get(chat_message.source.entity()) else {
            // TODO: Should probably disconnect
            continue;
        };

        let mut chat_history_update = messages::InterfaceTextBoxUpdate::new("chat/history");
        chat_history_update.append_line().with_text(
            format!("[{}] {}", &username, &chat_message.text),
            CHAT_FONT_SIZE,
            CHAT_TEXT_COLOR,
        );
        net.broadcast(chat_history_update);
    }
}

// TODO: Maybe players should be passed the chat history too.
// TODO: The "joined game" message sometimes shows for the player that joined. Intermitent problem,
// the message should arrive before the client finishes setup. In which case it should be
// discarded after two event buffer switches.
fn send_connection_messages(
    net: Res<NetworkServer>,
    username_query: Query<&Username>,
    mut network_events: EventReader<ServerNetworkEvent>,
) {
    for event in network_events.read() {
        match event {
            ServerNetworkEvent::Connected { entity } => {
                let mut chat_update = messages::InterfaceTextBoxUpdate::new("chat/history");
                let username = username_query.get(*entity).unwrap();
                chat_update.append_line().with_text(
                    format!("{} joined the game", username),
                    CHAT_FONT_SIZE,
                    CHAT_TEXT_COLOR,
                );
                net.broadcast(chat_update);
            }
            ServerNetworkEvent::Disconnected { entity } => {
                let username = username_query.get(*entity).unwrap();
                let mut chat_update = messages::InterfaceTextBoxUpdate::new("chat/history");
                chat_update.append_line().with_text(
                    format!("{} left the game", username),
                    CHAT_FONT_SIZE,
                    CHAT_TEXT_COLOR,
                );
                net.broadcast(chat_update);
            }
            _ => (),
        }
    }
}
