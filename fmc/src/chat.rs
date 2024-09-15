use bevy::prelude::*;
use fmc_protocol::messages;

use crate::{
    networking::{NetworkEvent, NetworkMessage, Server},
    players::Player,
};

pub const CHAT_FONT_SIZE: f32 = 8.0;
pub const CHAT_TEXT_COLOR: &str = "#ffffff";

pub struct ChatPlugin;
impl Plugin for ChatPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (handle_chat_messages, send_connection_messages));
    }
}

fn handle_chat_messages(
    net: Res<Server>,
    player_query: Query<&Player>,
    mut chat_message_query: EventReader<NetworkMessage<messages::InterfaceTextInput>>,
) {
    for chat_message in chat_message_query.read() {
        if &chat_message.interface_path != "chat/input" {
            continue;
        }
        let Ok(player) = player_query.get(chat_message.player_entity) else {
            // TODO: Should probably disconnect
            continue;
        };

        let mut chat_history_update = messages::InterfaceTextUpdate::new("chat/history");
        chat_history_update.append_line().with_text(
            format!("[{}] {}", &player.username, &chat_message.text),
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
    net: Res<Server>,
    player_query: Query<&Player>,
    mut network_events: EventReader<NetworkEvent>,
) {
    for event in network_events.read() {
        match event {
            NetworkEvent::Connected { entity } => {
                let mut chat_update = messages::InterfaceTextUpdate::new("chat/history");
                let player = &player_query.get(*entity).unwrap();
                chat_update.append_line().with_text(
                    format!("{} joined the game", player.username),
                    CHAT_FONT_SIZE,
                    CHAT_TEXT_COLOR,
                );
                net.broadcast(chat_update);
            }
            NetworkEvent::Disconnected { entity } => {
                let player = player_query.get(*entity).unwrap();
                let mut chat_update = messages::InterfaceTextUpdate::new("chat/history");
                chat_update.append_line().with_text(
                    format!("{} left the game", player.username),
                    CHAT_FONT_SIZE,
                    CHAT_TEXT_COLOR,
                );
                net.broadcast(chat_update);
            }
        }
    }
}
