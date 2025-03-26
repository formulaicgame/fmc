use std::{
    io::{Read, Write},
    net::{Shutdown, SocketAddr, TcpStream},
    path::PathBuf,
};

use bevy::{
    ecs::system::SystemParam,
    prelude::*,
    tasks::{futures_lite::future, AsyncComputeTaskPool, Task},
};
use fmc_protocol::{messages, MessageType, ServerBound};
use serde::Serialize;

use crate::{assets::AssetState, game_state::GameState};

// Message length (4 bytes)
const COMPRESSION_HEADER_SIZE: usize = 4;
// MessageType (1 byte) + message length (4 bytes)
const MESSAGE_HEADER_SIZE: usize = 5;

pub struct ClientPlugin;

impl Plugin for ClientPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Identity::read_from_file())
            .insert_resource(NetworkClient::new())
            .add_event::<messages::AssetResponse>()
            .add_event::<messages::Disconnect>()
            .add_event::<messages::ServerConfig>()
            .add_event::<messages::Time>()
            .add_event::<messages::Chunk>()
            .add_event::<messages::BlockUpdates>()
            .add_event::<messages::NewModel>()
            .add_event::<messages::DeleteModel>()
            .add_event::<messages::ModelPlayAnimation>()
            .add_event::<messages::ModelUpdateAsset>()
            .add_event::<messages::ModelUpdateTransform>()
            .add_event::<messages::ModelColor>()
            .add_event::<messages::SpawnCustomModel>()
            .add_event::<messages::PlayerAabb>()
            .add_event::<messages::PlayerCameraPosition>()
            .add_event::<messages::PlayerCameraRotation>()
            .add_event::<messages::PlayerPosition>()
            .add_event::<messages::InterfaceItemBoxUpdate>()
            .add_event::<messages::InterfaceNodeVisibilityUpdate>()
            .add_event::<messages::InterfaceTextUpdate>()
            .add_event::<messages::InterfaceVisibilityUpdate>()
            .add_event::<messages::EnableClientAudio>()
            .add_event::<messages::Sound>()
            .add_event::<messages::ParticleEffect>()
            .add_event::<messages::Plugin>()
            // Updated manually
            .init_resource::<Events<messages::PluginData>>()
            .add_systems(OnEnter(GameState::Playing), send_client_ready)
            .add_systems(
                PreUpdate,
                (
                    read_messages.run_if(in_state(GameState::Playing)),
                    (connect, initialize_connection).run_if(in_state(GameState::Connecting)),
                    (
                        register_client_disconnect_events.before(disconnect),
                        disconnect,
                    )
                        .run_if(not(in_state(GameState::Launcher))),
                ),
            );
    }
}

struct ConcurrentQueue {
    sender: crossbeam::Sender<String>,
    receiver: crossbeam::Receiver<String>,
}

impl ConcurrentQueue {
    fn new() -> Self {
        let (sender, receiver) = crossbeam::bounded(1);
        Self { sender, receiver }
    }

    fn push(&self, value: String) -> Result<(), String> {
        self.sender.try_send(value).map_err(|e| e.into_inner())
    }

    fn pop(&self) -> Result<String, ()> {
        self.receiver.try_recv().map_err(|e| ())
    }
}

// TODO: Implement the buffers as some Read/Write impl it's too much to keep track of.
#[derive(Resource)]
pub struct NetworkClient {
    connection: Option<TcpStream>,
    connection_task: Option<Task<std::io::Result<TcpStream>>>,
    disconnect_events: ConcurrentQueue,
    // buffer for connection reads, compressed
    read_buffer: Vec<u8>,
    read_cursor: usize,
    // Buffer for decompressed messages
    message_buffer: Vec<u8>,
    message_cursor: usize,
}

impl NetworkClient {
    fn new() -> Self {
        Self {
            connection: None,
            connection_task: None,
            disconnect_events: ConcurrentQueue::new(),
            read_buffer: Vec::new(),
            read_cursor: 0,
            message_buffer: Vec::new(),
            message_cursor: 0,
        }
    }

    pub fn connect(&mut self, addr: SocketAddr) {
        if self.connection.is_some() || self.connection_task.is_some() {
            panic!("Already connected");
        }

        self.connection_task = Some(AsyncComputeTaskPool::get().spawn(async move {
            TcpStream::connect_timeout(&addr, std::time::Duration::from_secs(10)).and_then(|tcp| {
                tcp.set_nonblocking(true)?;
                tcp.set_nodelay(true)?;
                Ok(tcp)
            })
        }));
    }

    pub fn send_message<T: ServerBound + Serialize>(&self, message: T) {
        let size = bincode::serialized_size(&message).unwrap() as u32;
        let mut serialized = vec![0; MESSAGE_HEADER_SIZE + size as usize];

        serialized[0] = T::TYPE as u8;
        serialized[1..5].copy_from_slice(&size.to_le_bytes());

        bincode::serialize_into(&mut serialized[5..], &message).unwrap();

        let mut connection = self.connection.as_ref().unwrap();
        match connection.write(&serialized) {
            Ok(_) => (),
            Err(_e) => {
                self.disconnect("connection lost");
            }
        }
    }

    pub fn disconnect<T: AsRef<str>>(&self, message: T) {
        if self.connection.is_none() && self.connection_task.is_none() {
            return;
        }

        println!("{}", message.as_ref());
        // Disconnect might be called many times as errors cascade, but only the first message will
        // register as that will be the primary cause. The event queue only has capacity for one
        // element.
        self.disconnect_events
            .push(message.as_ref().to_string())
            .ok();
    }

    fn is_connected(&self) -> bool {
        return self.connection.is_some();
    }

    fn read_packets(&mut self) {
        // Move the remaning content of the read buffer to the beginning.
        self.read_buffer.copy_within(self.read_cursor.., 0);
        self.read_buffer
            .truncate(self.read_buffer.len() - self.read_cursor);
        self.read_cursor = 0;

        let mut buf = [0u8; 65536];
        let connection = self.connection.as_mut().unwrap();
        loop {
            let size = match connection.read(&mut buf) {
                Ok(size) => size,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(e) => {
                    self.disconnect(e.kind().to_string());
                    break;
                }
            };

            if size == 0 {
                break;
            } else {
                self.read_buffer.extend_from_slice(&buf[..size]);
            }
        }
    }

    // Try to decompress a packet if there aren't any messages already available
    fn try_decompress_packet(&mut self) -> bool {
        if self.message_buffer.len() - self.message_cursor > MESSAGE_HEADER_SIZE {
            let message_size = u32::from_le_bytes(
                self.message_buffer
                    [self.message_cursor + 1..self.message_cursor + MESSAGE_HEADER_SIZE]
                    .try_into()
                    .unwrap(),
            ) as usize;

            // There is already a message available
            if message_size < self.message_buffer.len() - self.message_cursor + MESSAGE_HEADER_SIZE
            {
                return true;
            }
        }

        if self.read_buffer.len() - self.read_cursor <= COMPRESSION_HEADER_SIZE {
            return false;
        }

        let packet_length = u32::from_le_bytes(
            self.read_buffer[self.read_cursor..self.read_cursor + COMPRESSION_HEADER_SIZE]
                .try_into()
                .unwrap(),
        ) as usize;

        // Return if the packet hasn't arrived yet
        if packet_length > self.read_buffer.len() - self.read_cursor + COMPRESSION_HEADER_SIZE {
            return false;
        }

        self.read_cursor += COMPRESSION_HEADER_SIZE;

        let packet = &self.read_buffer[self.read_cursor..self.read_cursor + packet_length];

        if let Err(e) = zstd::stream::copy_decode(packet, &mut self.message_buffer) {
            error!("{}", e);
            self.disconnect("Corrupt network packet, failed to decompress");
            return false;
        }

        self.read_cursor += packet_length;

        return true;
    }

    // Extract a message from the message buffer
    fn extract_message<'a>(&'a mut self) -> Option<(MessageType, &'a [u8])> {
        if self.message_buffer.len() - self.message_cursor <= MESSAGE_HEADER_SIZE {
            // Move the partial message to the beginning of the buffer to make room for more bytes.
            self.message_buffer.copy_within(self.message_cursor.., 0);
            self.message_buffer
                .truncate(self.message_buffer.len() - self.message_cursor);
            self.message_cursor = 0;
            return None;
        }

        let message_type = self.message_buffer[self.message_cursor];
        if message_type >= MessageType::MAX as u8 {
            // Received invalid message type, return invalid message to disconnect
            return Some((MessageType::MAX, &[]));
        }
        let message_type: MessageType = unsafe { std::mem::transmute(message_type) };

        let message_length = u32::from_le_bytes(
            self.message_buffer[self.message_cursor + 1..self.message_cursor + MESSAGE_HEADER_SIZE]
                .try_into()
                .unwrap(),
        ) as usize;

        if message_length > self.message_buffer.len() - self.message_cursor + MESSAGE_HEADER_SIZE {
            self.message_buffer.copy_within(self.message_cursor.., 0);
            self.message_buffer
                .truncate(self.message_buffer.len() - self.message_cursor);
            self.message_cursor = 0;
            return None;
        }

        self.message_cursor += MESSAGE_HEADER_SIZE;

        let message =
            &self.message_buffer[self.message_cursor..self.message_cursor + message_length];
        self.message_cursor += message_length;

        return Some((message_type, message));
    }

    // Try to grab a message from the message buffer, if not possible, decompress and try again
    fn next_message<'a>(&'a mut self) -> Option<(MessageType, &'a [u8])> {
        if !self.try_decompress_packet() {
            return None;
        }

        return self.extract_message();
    }
}

fn send_client_ready(net: Res<NetworkClient>) {
    net.send_message(messages::ClientReady);
}

fn connect(mut net: ResMut<NetworkClient>, identity: Res<Identity>) {
    if let Some(Some(result)) = net
        .connection_task
        .as_mut()
        .map(|task| future::block_on(future::poll_once(task)))
    {
        match result {
            Ok(tcp_stream) => {
                net.connection = Some(tcp_stream);
                net.send_message(messages::ClientIdentification {
                    name: identity.username.clone(),
                });
            }
            Err(e) => net.disconnect(e.kind().to_string()),
        };

        net.connection_task.take();
    }
}

// After the client identifies itself, the server will send a server config. If we already have the
// assets the server config points to, we immediately start to load, else we request them from the
// server.
fn initialize_connection(
    mut commands: Commands,
    mut net: ResMut<NetworkClient>,
    mut asset_state: ResMut<NextState<AssetState>>,
    server_config: Option<Res<messages::ServerConfig>>,
    mut downloading_assets: Local<bool>,
) {
    if net.connection.is_none() {
        return;
    }
    net.read_packets();

    if net.read_buffer.len() < COMPRESSION_HEADER_SIZE {
        return;
    }

    if *downloading_assets {
        if let Some((message_type, message_data)) = net.next_message() {
            *downloading_assets = false;

            let Ok(asset_response) = bincode::deserialize::<messages::AssetResponse>(message_data)
            else {
                net.disconnect(format!(
                    "The server sent a {:?} message, when it should have sent an AssetResponse message.",
                    message_type
                ));
                return;
            };

            let server_config = server_config.unwrap();

            let asset_hash_hex = format!("{:x}", server_config.assets_hash);
            let path = PathBuf::from("./server_assets").join(&asset_hash_hex);

            // Create directories, silently fails if they already exist
            std::fs::create_dir("./server_assets").ok();
            std::fs::create_dir(&path).ok();

            // We symlink the wanted asset path to "server_assets/active" so that all other parts
            // of the program can assume the assets are located at a static location, even though
            // they are switched out for each server we connect to. As opposed to registering the
            // path as a variable and having to pass it around.
            const LINK_PATH: &str = "./server_assets/active";
            std::fs::remove_dir_all(LINK_PATH).ok();

            #[cfg(target_family = "windows")]
            {
                std::os::windows::fs::symlink_dir(&asset_hash_hex, LINK_PATH);
            }
            #[cfg(target_family = "unix")]
            {
                std::os::unix::fs::symlink(&asset_hash_hex, LINK_PATH).unwrap();
            }
            #[cfg(target_family = "wasm")]
            {
                // This is available as a nightly api under std::os::wasi
                compile_error!("Not implemented for wasm yet");
            }

            let mut archive = tar::Archive::new(asset_response.file.as_slice());

            if let Err(e) = archive.unpack(LINK_PATH) {
                net.disconnect(e.to_string());
                return;
            }

            asset_state.set(AssetState::Loading);
        }
    } else {
        if let Some((message_type, message_data)) = net.next_message() {
            let Ok(server_config) = bincode::deserialize::<messages::ServerConfig>(message_data)
            else {
                net.disconnect(format!(
                    "The server sent a {:?} message, when it should have sent a ServerConfig message.",
                    message_type
                ));
                return;
            };

            // convert u64 hash to hex so it's more manageable as a file path
            let asset_hash_hex = format!("{:x}", server_config.assets_hash);
            let path = PathBuf::from("./server_assets").join(&asset_hash_hex);

            if path.exists() {
                asset_state.set(AssetState::Loading);
            } else {
                *downloading_assets = true;
                net.send_message(messages::AssetRequest);
            }

            commands.insert_resource(server_config);
        }
    }
}

// Even though we disconnect through a client error we want to register the reason it
// disconnected as an event so that it can be displayed to the user. We register it as a server
// message to piggyback of the same code path as a disconnection initiated by the server.
fn register_client_disconnect_events(
    net: Res<NetworkClient>,
    mut network_events: EventWriter<messages::Disconnect>,
) {
    if let Ok(message) = net.disconnect_events.pop() {
        network_events.send(messages::Disconnect { message });
    }
}

fn disconnect(
    mut net: ResMut<NetworkClient>,
    mut game_state: ResMut<NextState<GameState>>,
    mut disconnect_events: EventReader<messages::Disconnect>,
) {
    for _ in disconnect_events.read() {
        if let Some(connection) = net.connection.take() {
            connection.shutdown(Shutdown::Both).ok();
        }

        // Tasks are canceled when dropped (eventually)
        net.connection_task.take();

        game_state.set(GameState::Launcher);
    }
}

#[derive(Resource)]
pub struct Identity {
    pub username: String,
}

impl Identity {
    fn read_from_file() -> Self {
        if let Ok(username) = std::fs::read_to_string("./identity.txt") {
            Identity {
                username: username.trim().to_owned(),
            }
        } else {
            Identity {
                username: String::new(),
            }
        }
    }

    pub fn is_valid(&self) -> bool {
        !self.username.is_empty()
    }
}

// TODO: Write a macro for all this
#[derive(SystemParam)]
struct EventWriters<'w> {
    asset_response: EventWriter<'w, messages::AssetResponse>,
    disconnect: EventWriter<'w, messages::Disconnect>,
    server_config: EventWriter<'w, messages::ServerConfig>,
    time: EventWriter<'w, messages::Time>,
    chunk: EventWriter<'w, messages::Chunk>,
    block_updates: EventWriter<'w, messages::BlockUpdates>,
    new_model: EventWriter<'w, messages::NewModel>,
    delete_model: EventWriter<'w, messages::DeleteModel>,
    model_play_animation: EventWriter<'w, messages::ModelPlayAnimation>,
    model_update_asset: EventWriter<'w, messages::ModelUpdateAsset>,
    model_update_transform: EventWriter<'w, messages::ModelUpdateTransform>,
    spawn_custom_model: EventWriter<'w, messages::SpawnCustomModel>,
    model_color: EventWriter<'w, messages::ModelColor>,
    player_aabb: EventWriter<'w, messages::PlayerAabb>,
    player_camera_position: EventWriter<'w, messages::PlayerCameraPosition>,
    player_camera_rotation: EventWriter<'w, messages::PlayerCameraRotation>,
    player_position: EventWriter<'w, messages::PlayerPosition>,
    interface_item_box_update: EventWriter<'w, messages::InterfaceItemBoxUpdate>,
    interface_node_visibility_update: EventWriter<'w, messages::InterfaceNodeVisibilityUpdate>,
    interface_text_update: EventWriter<'w, messages::InterfaceTextUpdate>,
    interface_visibility_update: EventWriter<'w, messages::InterfaceVisibilityUpdate>,
    enable_client_audio: EventWriter<'w, messages::EnableClientAudio>,
    sound: EventWriter<'w, messages::Sound>,
    particle_effect: EventWriter<'w, messages::ParticleEffect>,
    plugin: EventWriter<'w, messages::Plugin>,
    plugin_data: EventWriter<'w, messages::PluginData>,
}

fn read_messages(net: ResMut<NetworkClient>, mut event_writers: EventWriters) {
    if !net.is_connected() {
        return;
    }

    // For split borrows
    let net = net.into_inner();

    net.read_packets();

    while let Some((message_type, message_data)) = net.next_message() {
        match message_type {
            MessageType::AssetResponse => {
                if let Ok(message) = bincode::deserialize(message_data) {
                    event_writers.asset_response.send(message);
                    continue;
                }
            }
            MessageType::Disconnect => {
                if let Ok(message) = bincode::deserialize(message_data) {
                    event_writers.disconnect.send(message);
                    continue;
                }
            }
            MessageType::ServerConfig => {
                if let Ok(message) = bincode::deserialize(message_data) {
                    event_writers.server_config.send(message);
                    continue;
                }
            }
            MessageType::Time => {
                if let Ok(message) = bincode::deserialize(message_data) {
                    event_writers.time.send(message);
                    continue;
                }
            }
            MessageType::Chunk => {
                if let Ok(message) = bincode::deserialize(message_data) {
                    event_writers.chunk.send(message);
                    continue;
                }
            }
            MessageType::BlockUpdates => {
                if let Ok(message) = bincode::deserialize(message_data) {
                    event_writers.block_updates.send(message);
                    continue;
                }
            }
            MessageType::NewModel => {
                if let Ok(message) = bincode::deserialize(message_data) {
                    event_writers.new_model.send(message);
                    continue;
                }
            }
            MessageType::DeleteModel => {
                if let Ok(message) = bincode::deserialize(message_data) {
                    event_writers.delete_model.send(message);
                    continue;
                }
            }
            MessageType::ModelPlayAnimation => {
                if let Ok(message) = bincode::deserialize(message_data) {
                    event_writers.model_play_animation.send(message);
                    continue;
                }
            }
            MessageType::ModelUpdateAsset => {
                if let Ok(message) = bincode::deserialize(message_data) {
                    event_writers.model_update_asset.send(message);
                    continue;
                }
            }
            MessageType::ModelUpdateTransform => {
                if let Ok(message) = bincode::deserialize(message_data) {
                    event_writers.model_update_transform.send(message);
                    continue;
                }
            }
            MessageType::SpawnCustomModel => {
                if let Ok(message) = bincode::deserialize(message_data) {
                    event_writers.spawn_custom_model.send(message);
                    continue;
                }
            }
            MessageType::ModelColor => {
                if let Ok(message) = bincode::deserialize(message_data) {
                    event_writers.model_color.send(message);
                    continue;
                }
            }
            MessageType::PlayerAabb => {
                if let Ok(message) = bincode::deserialize(message_data) {
                    event_writers.player_aabb.send(message);
                    continue;
                }
            }
            MessageType::PlayerCameraPosition => {
                if let Ok(message) = bincode::deserialize(message_data) {
                    event_writers.player_camera_position.send(message);
                    continue;
                }
            }
            MessageType::PlayerCameraRotation => {
                if let Ok(message) = bincode::deserialize(message_data) {
                    event_writers.player_camera_rotation.send(message);
                    continue;
                }
            }
            MessageType::PlayerPosition => {
                if let Ok(message) = bincode::deserialize(message_data) {
                    event_writers.player_position.send(message);
                    continue;
                }
            }
            MessageType::InterfaceItemBoxUpdate => {
                if let Ok(message) = bincode::deserialize(message_data) {
                    event_writers.interface_item_box_update.send(message);
                    continue;
                }
            }
            MessageType::InterfaceNodeVisibilityUpdate => {
                if let Ok(message) = bincode::deserialize(message_data) {
                    event_writers.interface_node_visibility_update.send(message);
                    continue;
                }
            }
            MessageType::InterfaceTextUpdate => {
                if let Ok(message) = bincode::deserialize(message_data) {
                    event_writers.interface_text_update.send(message);
                    continue;
                }
            }
            MessageType::InterfaceVisibilityUpdate => {
                if let Ok(message) = bincode::deserialize(message_data) {
                    event_writers.interface_visibility_update.send(message);
                    continue;
                }
            }
            MessageType::EnableClientAudio => {
                if let Ok(message) = bincode::deserialize(message_data) {
                    event_writers.enable_client_audio.send(message);
                    continue;
                }
            }
            MessageType::Sound => {
                if let Ok(message) = bincode::deserialize(message_data) {
                    event_writers.sound.send(message);
                    continue;
                }
            }
            MessageType::ParticleEffect => {
                if let Ok(message) = bincode::deserialize(message_data) {
                    event_writers.particle_effect.send(message);
                    continue;
                }
            }
            MessageType::Plugin => {
                if let Ok(message) = bincode::deserialize(message_data) {
                    event_writers.plugin.send(message);
                    continue;
                }
            }
            MessageType::PluginData => {
                if let Ok(message) = bincode::deserialize(message_data) {
                    event_writers.plugin_data.send(message);
                    continue;
                }
            }
            _ => {
                net.disconnect(format!(
                    "Corrupt network message, received invalid message type: {:?}",
                    message_type
                ));
                break;
            }
        }

        net.disconnect(format!("Corrupt network message, received message type {:?} but it did not correspond to the data.", message_type));
        break;
    }
}
