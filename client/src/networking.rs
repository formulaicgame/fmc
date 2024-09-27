use std::{
    io::{Read, Write},
    net::{Shutdown, SocketAddr, TcpStream},
    path::{Path, PathBuf},
};

use bevy::{
    ecs::system::SystemParam,
    prelude::*,
    tasks::{futures_lite::future, AsyncComputeTaskPool, Task},
};
use concurrent_queue::ConcurrentQueue;
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
            .add_event::<messages::DeleteModel>()
            .add_event::<messages::ModelPlayAnimation>()
            .add_event::<messages::ModelUpdateAsset>()
            .add_event::<messages::ModelUpdateTransform>()
            .add_event::<messages::NewModel>()
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

// TODO: Implement the buffers as some Read/Write impl it's too much to keep track of.
#[derive(Resource)]
pub struct NetworkClient {
    connection: Option<TcpStream>,
    connection_task: Option<Task<std::io::Result<TcpStream>>>,
    disconnect_events: ConcurrentQueue<String>,
    // buffer for connection reads, compressed
    read_buffer: Vec<u8>,
    read_cursor: usize,
    read_bytes: usize,
    // Buffer for decompressed messages
    message_buffer: Vec<u8>,
    message_cursor: usize,
    message_bytes: usize,
}

impl NetworkClient {
    fn new() -> Self {
        Self {
            connection: None,
            connection_task: None,
            disconnect_events: ConcurrentQueue::bounded(1),
            read_buffer: vec![0; 1024 * 1024],
            read_cursor: 0,
            read_bytes: 0,
            message_buffer: vec![0; 1024 * 1024],
            message_cursor: 0,
            message_bytes: 0,
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

    #[track_caller]
    pub fn disconnect<T: AsRef<str>>(&self, message: T) {
        if self.connection.is_none() && self.connection_task.is_none() {
            return;
        }

        dbg!(message.as_ref());
        // Disconnect might be called many times as errors cascade, but only the first message will
        // register as that will be the primary cause. The event queue is only has capacity for one
        // element.
        self.disconnect_events
            .push(message.as_ref().to_string())
            .ok();
    }

    fn is_connected(&self) -> bool {
        return self.connection.is_some();
    }

    fn is_connecting(&self) -> bool {
        return self.connection_task.is_some();
    }

    fn read_packets(&mut self) {
        // Move the remaning content of the read buffer to the beginning.
        self.read_buffer
            .copy_within(self.read_cursor..self.read_bytes, 0);
        self.read_bytes -= self.read_cursor;
        self.read_cursor = 0;

        let connection = self.connection.as_mut().unwrap();
        let size = match connection.read(&mut self.read_buffer[self.read_bytes..]) {
            Ok(size) => size,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => return,
            Err(e) => {
                self.disconnect(e.kind().to_string());
                return;
            }
        };
        self.read_bytes += size;
    }

    // Try to decompress a packet if there aren't any messages already available
    fn try_decompress_packet(&mut self) -> bool {
        if self.message_bytes - self.message_cursor > MESSAGE_HEADER_SIZE {
            let message_size = u32::from_le_bytes(
                self.message_buffer
                    [self.message_cursor + 1..self.message_cursor + MESSAGE_HEADER_SIZE]
                    .try_into()
                    .unwrap(),
            ) as usize;

            // There is already a message available
            if message_size < self.message_bytes - self.message_cursor + MESSAGE_HEADER_SIZE {
                return true;
            }
        }

        if self.read_bytes - self.read_cursor <= COMPRESSION_HEADER_SIZE {
            return false;
        }

        let packet_length = u32::from_le_bytes(
            self.read_buffer[self.read_cursor..self.read_cursor + COMPRESSION_HEADER_SIZE]
                .try_into()
                .unwrap(),
        ) as usize;

        // Return if the packet hasn't arrived yet
        if packet_length > self.read_bytes - self.read_cursor + COMPRESSION_HEADER_SIZE {
            return false;
        }

        // Shift remaining message bytes to beginning to prepare for writing
        self.message_buffer
            .copy_within(self.message_cursor..self.message_bytes, 0);
        self.message_bytes -= self.message_cursor;
        self.message_cursor = 0;

        self.read_cursor += COMPRESSION_HEADER_SIZE;

        // This is a copy of zstd::stream::copy_decode, but it returns how many bytes were written
        fn decode_all(from: &[u8], mut to: &mut [u8]) -> std::io::Result<usize> {
            let mut decoder = zstd::Decoder::new(from)?;
            let written = std::io::copy(&mut decoder, &mut to)?;
            return Ok(written as usize);
        }

        let packet = &self.read_buffer[self.read_cursor..self.read_cursor + packet_length];
        let message_buffer = &mut self.message_buffer[self.message_cursor..];
        let decoded_size = match decode_all(packet, message_buffer) {
            Ok(size) => size,
            Err(e) => {
                error!("{}", e);
                self.disconnect("Corrupted network packet, failed to decompress");
                return false;
            }
        };
        self.read_cursor += packet_length;
        self.message_bytes += decoded_size;

        return true;
    }

    // Extract a message from the message buffer
    fn extract_message<'a>(&'a mut self) -> Option<(MessageType, &'a [u8])> {
        if self.message_bytes - self.message_cursor <= MESSAGE_HEADER_SIZE {
            // Move the partial message to the beginning of the buffer to make room for more bytes.
            self.message_buffer
                .copy_within(self.message_cursor..self.message_bytes, 0);
            self.message_bytes -= self.message_cursor;
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

        if message_length > self.message_bytes - self.message_cursor + MESSAGE_HEADER_SIZE {
            self.message_buffer
                .copy_within(self.message_cursor..self.message_bytes, 0);
            self.message_bytes -= self.message_cursor;
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

#[derive(Default)]
struct AssetDownload {
    // Size of the compressed assets, first thing the server sends
    size: usize,
    // How much have been downloaded
    downloaded: usize,
    // Buffer for downloaded data
    data: Option<Vec<u8>>,
    // Final path to unpack assets into
    path: Option<PathBuf>,
}

// After the client identifies itself, the server will send a server config. If we already have the
// assets the server config points to, we immediately start to load, else we request them from the
// server.
fn initialize_connection(
    mut commands: Commands,
    mut net: ResMut<NetworkClient>,
    mut asset_download: Local<AssetDownload>,
    mut asset_state: ResMut<NextState<AssetState>>,
) {
    if net.connection.is_none() {
        return;
    }
    net.read_packets();

    if net.read_bytes < MESSAGE_HEADER_SIZE {
        return;
    }

    if asset_download.data.is_some() {
        let mut cursor = 0;
        if asset_download.size == 0 {
            asset_download.size =
                u32::from_le_bytes(net.read_buffer[..4].try_into().unwrap()) as usize;
            cursor = 4;
        }

        asset_download
            .data
            .as_mut()
            .unwrap()
            .write(&net.read_buffer[cursor..net.read_bytes])
            .unwrap();
        asset_download.downloaded += net.read_bytes - cursor;
        net.read_bytes = 0;

        if asset_download.size == asset_download.downloaded {
            let data = asset_download.data.take().unwrap();
            let decoder = zstd::Decoder::new(&data[..]).unwrap();
            let mut archive = tar::Archive::new(decoder);

            let path = asset_download.path.take().unwrap();
            if let Err(e) = archive.unpack(&path) {
                net.disconnect(e.to_string());
                return;
            }

            set_active_asset_folder(&path);
            asset_state.set(AssetState::Loading);
        } else if asset_download.size < asset_download.downloaded {
            net.disconnect(format!(
                "Server sent too much asset data, expected {} bytes, but got {}",
                asset_download.size, asset_download.downloaded
            ));
        }
    } else {
        if let Some((message_type, message_data)) = net.next_message() {
            let Ok(server_config) = bincode::deserialize::<messages::ServerConfig>(message_data)
            else {
                net.disconnect(format!(
                    "The server sent a {:?} message, when it should have sent a server config.",
                    message_type
                ));
                return;
            };
            net.read_bytes = 0;
            net.read_cursor = 0;

            // Absolute path because symlinks are parsed as relative to the symlink path instead of
            // relative to the execution path. Symlink would point "./server_config/active" ->
            // "./server_config/active/server_config/hash" if kept relative.
            let path = PathBuf::from("server_assets")
                .canonicalize()
                .unwrap()
                .join(&format!("{:x}", server_config.assets_hash));
            if path.exists() {
                set_active_asset_folder(&path);
                asset_state.set(AssetState::Loading);
            } else {
                asset_download.data = Some(Vec::new());
                asset_download.path = Some(path);
                net.send_message(messages::AssetRequest);
            }
            commands.insert_resource(server_config);
        }
    }
}

// It's unwieldy to register the asset path everywhere, much easier to just always load from the
// same place. So we create a symlink "server_assets/some-hash" -> "server_assets/active"
fn set_active_asset_folder(path: &Path) {
    const LINK_PATH: &str = "./server_assets/active";

    std::fs::remove_dir_all(LINK_PATH).ok();

    #[cfg(target_family = "windows")]
    {
        std::os::windows::fs::symlink_dir(&path, LINK_PATH);
    }
    #[cfg(target_family = "unix")]
    {
        std::os::unix::fs::symlink(&path, LINK_PATH).unwrap();
    }
    #[cfg(target_family = "wasm")]
    {
        // This is available as a nightly api under std::os::wasi
        compile_error!("Not implemented for wasm yet");
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
        if !net.is_connected() && !net.is_connecting() {
            continue;
        }

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

// All ClientBound messages
#[derive(SystemParam)]
struct EventWriters<'w> {
    asset_response: EventWriter<'w, messages::AssetResponse>,
    disconnect: EventWriter<'w, messages::Disconnect>,
    server_config: EventWriter<'w, messages::ServerConfig>,
    time: EventWriter<'w, messages::Time>,
    chunk: EventWriter<'w, messages::Chunk>,
    block_updates: EventWriter<'w, messages::BlockUpdates>,
    delete_model: EventWriter<'w, messages::DeleteModel>,
    model_play_animation: EventWriter<'w, messages::ModelPlayAnimation>,
    model_update_asset: EventWriter<'w, messages::ModelUpdateAsset>,
    model_update_transform: EventWriter<'w, messages::ModelUpdateTransform>,
    new_model: EventWriter<'w, messages::NewModel>,
    spawn_custom_model: EventWriter<'w, messages::SpawnCustomModel>,
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
                } else {
                    net.disconnect(format!("Corrupt network message, received message type {:?} but it did not correspond to the data.", message_type));
                    break;
                }
            }
            MessageType::Disconnect => {
                if let Ok(message) = bincode::deserialize(message_data) {
                    event_writers.disconnect.send(message);
                } else {
                    net.disconnect(format!("Corrupt network message, received message type {:?} but it did not correspond to the data.", message_type));
                    break;
                }
            }
            MessageType::ServerConfig => {
                if let Ok(message) = bincode::deserialize(message_data) {
                    event_writers.server_config.send(message);
                } else {
                    net.disconnect(format!("Corrupt network message, received message type {:?} but it did not correspond to the data.", message_type));
                    break;
                }
            }
            MessageType::Time => {
                if let Ok(message) = bincode::deserialize(message_data) {
                    event_writers.time.send(message);
                } else {
                    net.disconnect(format!("Corrupt network message, received message type {:?} but it did not correspond to the data.", message_type));
                    break;
                }
            }
            MessageType::Chunk => {
                if let Ok(message) = bincode::deserialize(message_data) {
                    event_writers.chunk.send(message);
                } else {
                    net.disconnect(format!("Corrupt network message, received message type {:?} but it did not correspond to the data.", message_type));
                    break;
                }
            }
            MessageType::BlockUpdates => {
                if let Ok(message) = bincode::deserialize(message_data) {
                    event_writers.block_updates.send(message);
                } else {
                    net.disconnect(format!("Corrupt network message, received message type {:?} but it did not correspond to the data.", message_type));
                    break;
                }
            }
            MessageType::DeleteModel => {
                if let Ok(message) = bincode::deserialize(message_data) {
                    event_writers.delete_model.send(message);
                } else {
                    net.disconnect(format!("Corrupt network message, received message type {:?} but it did not correspond to the data.", message_type));
                    break;
                }
            }
            MessageType::ModelPlayAnimation => {
                if let Ok(message) = bincode::deserialize(message_data) {
                    event_writers.model_play_animation.send(message);
                } else {
                    net.disconnect(format!("Corrupt network message, received message type {:?} but it did not correspond to the data.", message_type));
                    break;
                }
            }
            MessageType::ModelUpdateAsset => {
                if let Ok(message) = bincode::deserialize(message_data) {
                    event_writers.model_update_asset.send(message);
                } else {
                    net.disconnect(format!("Corrupt network message, received message type {:?} but it did not correspond to the data.", message_type));
                    break;
                }
            }
            MessageType::ModelUpdateTransform => {
                if let Ok(message) = bincode::deserialize(message_data) {
                    event_writers.model_update_transform.send(message);
                } else {
                    net.disconnect(format!("Corrupt network message, received message type {:?} but it did not correspond to the data.", message_type));
                    break;
                }
            }
            MessageType::NewModel => {
                if let Ok(message) = bincode::deserialize(message_data) {
                    event_writers.new_model.send(message);
                } else {
                    net.disconnect(format!("Corrupt network message, received message type {:?} but it did not correspond to the data.", message_type));
                    break;
                }
            }
            MessageType::SpawnCustomModel => {
                if let Ok(message) = bincode::deserialize(message_data) {
                    event_writers.spawn_custom_model.send(message);
                } else {
                    net.disconnect(format!("Corrupt network message, received message type {:?} but it did not correspond to the data.", message_type));
                    break;
                }
            }
            MessageType::PlayerAabb => {
                if let Ok(message) = bincode::deserialize(message_data) {
                    event_writers.player_aabb.send(message);
                } else {
                    net.disconnect(format!("Corrupt network message, received message type {:?} but it did not correspond to the data.", message_type));
                    break;
                }
            }
            MessageType::PlayerCameraPosition => {
                if let Ok(message) = bincode::deserialize(message_data) {
                    event_writers.player_camera_position.send(message);
                } else {
                    net.disconnect(format!("Corrupt network message, received message type {:?} but it did not correspond to the data.", message_type));
                    break;
                }
            }
            MessageType::PlayerCameraRotation => {
                if let Ok(message) = bincode::deserialize(message_data) {
                    event_writers.player_camera_rotation.send(message);
                } else {
                    net.disconnect(format!("Corrupt network message, received message type {:?} but it did not correspond to the data.", message_type));
                    break;
                }
            }
            MessageType::PlayerPosition => {
                if let Ok(message) = bincode::deserialize(message_data) {
                    event_writers.player_position.send(message);
                } else {
                    net.disconnect(format!("Corrupt network message, received message type {:?} but it did not correspond to the data.", message_type));
                    break;
                }
            }
            MessageType::InterfaceItemBoxUpdate => {
                if let Ok(message) = bincode::deserialize(message_data) {
                    event_writers.interface_item_box_update.send(message);
                } else {
                    net.disconnect(format!("Corrupt network message, received message type {:?} but it did not correspond to the data.", message_type));
                    break;
                }
            }
            MessageType::InterfaceNodeVisibilityUpdate => {
                if let Ok(message) = bincode::deserialize(message_data) {
                    event_writers.interface_node_visibility_update.send(message);
                } else {
                    net.disconnect(format!("Corrupt network message, received message type {:?} but it did not correspond to the data.", message_type));
                    break;
                }
            }
            MessageType::InterfaceTextUpdate => {
                if let Ok(message) = bincode::deserialize(message_data) {
                    event_writers.interface_text_update.send(message);
                } else {
                    net.disconnect(format!("Corrupt network message, received message type {:?} but it did not correspond to the data.", message_type));
                    break;
                }
            }
            MessageType::InterfaceVisibilityUpdate => {
                if let Ok(message) = bincode::deserialize(message_data) {
                    event_writers.interface_visibility_update.send(message);
                } else {
                    net.disconnect(format!("Corrupt network message, received message type {:?} but it did not correspond to the data.", message_type));
                    break;
                }
            }
            MessageType::EnableClientAudio => {
                if let Ok(message) = bincode::deserialize(message_data) {
                    event_writers.enable_client_audio.send(message);
                } else {
                    net.disconnect(format!("Corrupt network message, received message type {:?} but it did not correspond to the data.", message_type));
                    break;
                }
            }
            MessageType::Sound => {
                if let Ok(message) = bincode::deserialize(message_data) {
                    event_writers.sound.send(message);
                } else {
                    net.disconnect(format!("Corrupt network message, received message type {:?} but it did not correspond to the data.", message_type));
                    break;
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
    }
}
