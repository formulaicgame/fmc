use std::{
    collections::{HashMap, HashSet},
    io::{Read, Write},
    net::{SocketAddr, TcpStream},
    ops::{Range, RangeTo},
    sync::{
        Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};

use bevy::{ecs::system::SystemParam, platform::cell::SyncUnsafeCell};
use fmc_protocol::{ClientBound, MessageType, messages};
use serde::Serialize;

use crate::{
    assets::Assets,
    blocks::Blocks,
    items::Items,
    models::Models,
    particle_effects::ParticleEffects,
    players::{DefaultPlayerBundle, Player},
    prelude::*,
    world::MaxRenderDistance,
};

// Size of each connection's write buffer
const WRITE_BUFFER_SIZE: usize = 1024 * 1024;
const WRITE_BUFFER_MAX_SIZE: usize = 32 * 1024 * 1024;
// Size of each connection's read buffer, also caps the size of inbound messages
const READ_BUFFER_SIZE: usize = 1024;
// MessageType (1 byte) + message length (4 bytes)
const HEADER_SIZE: usize = 5;
// message length (4 bytes)
const COMPRESSION_HEADER: usize = 4;

/// Enables and manages the network interface
pub struct ServerPlugin;
impl Plugin for ServerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, server_setup)
            .add_message::<NetworkEvent>()
            .add_message::<NetworkMessage<messages::LeftClick>>()
            .add_message::<NetworkMessage<messages::RightClick>>()
            .add_message::<NetworkMessage<messages::RenderDistance>>()
            .add_message::<NetworkMessage<messages::PlayerCameraRotation>>()
            .add_message::<NetworkMessage<messages::PlayerPosition>>()
            .add_message::<NetworkMessage<messages::InterfaceEquipItem>>()
            .add_message::<NetworkMessage<messages::InterfaceInteraction>>()
            .add_message::<NetworkMessage<messages::InterfaceTextInput>>()
            .add_message::<NetworkMessage<messages::GuiSetting>>()
            .add_systems(First, read_messages)
            .add_systems(
                PreUpdate,
                // XXX: Remember that new connnections should always be added after messages are
                // read so that the server has one tick to register components to the player it
                // needs to handle messages.
                handle_new_connections,
            )
            .add_systems(Update, log_connections)
            .add_systems(
                Last,
                (
                    // Chained for these properties:
                    // 1. Player entities are removed a tick after they are disconnected. Lets you
                    //    save player data.
                    // 2. Sending before disconnecting so that the disconnection reason is
                    //    sent.
                    remove_disconnected_player_entities,
                    send_messages,
                    disconnect_players,
                )
                    .chain(),
            );
    }
}

fn server_setup(mut commands: Commands) {
    let socket_address: SocketAddr = "0.0.0.0:42069".parse().unwrap();

    let listener = std::net::TcpListener::bind(socket_address).unwrap();
    listener.set_nonblocking(true).unwrap();

    let server = Server {
        listener,
        connections: HashMap::new(),
        to_disconnect: Disconnections::default(),
        compression_buffer: vec![0; WRITE_BUFFER_SIZE],
        safe: AtomicBool::new(false),
    };

    commands.insert_resource(server);

    info!("Started listening for new connections!");
}

// This is wrapped to allow for split borrowing
#[derive(Default, Deref, DerefMut)]
struct Disconnections(Mutex<HashSet<Entity>>);

impl Disconnections {
    fn insert(&self, connection_entity: Entity) -> bool {
        self.lock().unwrap().insert(connection_entity)
    }
}

/// The main network interface.
#[derive(Resource)]
pub struct Server {
    listener: std::net::TcpListener,
    connections: HashMap<Entity, Connection>,
    to_disconnect: Disconnections,
    compression_buffer: Vec<u8>,
    safe: AtomicBool,
}

impl Server {
    /// Send a message to one client
    #[track_caller]
    pub fn send_one<T: ClientBound + Serialize>(&self, connection_entity: Entity, message: T) {
        self.send_many(&[connection_entity], message);
    }

    /// Send a message to many clients
    #[track_caller]
    pub fn send_many<'a, T: ClientBound + Serialize>(
        &self,
        connection_entities: impl IntoIterator<Item = &'a Entity>,
        message: T,
    ) {
        assert!(
            self.safe.load(Ordering::Relaxed) == true,
            "Messages can only be sent between 'First' and 'Last'"
        );

        for connection_entity in connection_entities.into_iter().cloned() {
            let Some(connection) = self.connections.get(&connection_entity) else {
                // TODO: Server isn't supposed to have access to removed connection entities?
                // Continue added to deal with later
                continue;
            };

            if connection.write_message(&message).is_err() {
                if self.disconnect(connection_entity) {
                    error!(
                        "Failed to send message, the player's write buffer is at capacity. Server is \
                        sending too much, or the connection is too slow. Disconnecting to prevent the \
                        client from being left in an unsynchronised state."
                    );
                };
            };
        }
    }

    /// Broadcast a message to all the clients
    #[track_caller]
    pub fn broadcast<'a, T: ClientBound + Serialize>(&self, message: T) {
        self.send_many(self.connections.keys(), message);
    }

    /// Disconnect a client
    pub fn disconnect(&self, connection_entity: Entity) -> bool {
        self.to_disconnect.insert(connection_entity)
    }
}

struct WriteBuffer(SyncUnsafeCell<Vec<u8>>);

impl WriteBuffer {
    fn new() -> Self {
        Self(SyncUnsafeCell::new(vec![0; WRITE_BUFFER_SIZE]))
    }

    fn capacity(&self) -> usize {
        unsafe { (*(self.0.get())).capacity() }
    }

    // TODO: This could be implemented with SliceIndex I think, but it requires an unstable flag
    // and I somewhat prefer having all the methods, makes you aware you are doing something
    // dangerous.
    fn index(&self, index: usize) -> &mut u8 {
        unsafe {
            let vec = &mut *self.0.get();
            &mut vec[index]
        }
    }

    fn range(&self, index: Range<usize>) -> &mut [u8] {
        unsafe {
            let vec = &mut *self.0.get();
            &mut vec[index]
        }
    }

    #[track_caller]
    fn range_to(&self, index: RangeTo<usize>) -> &mut [u8] {
        unsafe {
            let vec = &mut *self.0.get();
            &mut vec[index]
        }
    }
}

struct Connection {
    socket: TcpStream,
    address: SocketAddr,
    write_buffer: WriteBuffer,
    read_buffer: [u8; READ_BUFFER_SIZE],
    read_cursor: usize,
    read_bytes: usize,
    write_cursor: AtomicUsize,
    // Surge protection, holds messages that don't fit in the write buffer. Emptied each time
    // messages are sent so the allocation isn't permanent.
    overflow: Mutex<Vec<u8>>,
}

impl Connection {
    fn new(socket: TcpStream, address: SocketAddr) -> Self {
        Self {
            socket,
            address,
            write_buffer: WriteBuffer::new(),
            read_buffer: [0; READ_BUFFER_SIZE],
            read_cursor: 0,
            read_bytes: 0,
            write_cursor: AtomicUsize::new(0),
            overflow: Mutex::new(Vec::new()),
        }
    }

    fn read_from_socket(&mut self) -> std::io::Result<usize> {
        // Move any partially received message to the front to make room
        self.read_buffer
            .copy_within(self.read_cursor..self.read_bytes, 0);
        self.read_bytes -= self.read_cursor;
        self.read_cursor = 0;

        match self.socket.read(&mut self.read_buffer[self.read_bytes..]) {
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => Ok(0),
            Ok(size) => {
                self.read_bytes += size;
                Ok(size)
            }
            e => e,
        }
    }

    fn write_message<T: ClientBound + Serialize>(&self, message: &T) -> Result<(), ()> {
        let size = bincode::serialized_size(message).unwrap() as usize;
        let total_size = size + HEADER_SIZE;

        let mut cursor = self.write_cursor.load(Ordering::Relaxed);
        loop {
            if cursor + total_size > self.write_buffer.capacity() {
                // Surge protection, it should happen rarely. The WRITE_BUFFER_SIZE should be
                // set to a value that covers 99% of operation. Happens for example when a player
                // gets spawned at a spot where all the chunks are already loaded, and get sent at
                // the same time.
                // The allocated memory is freed again when the messages are sent.
                let mut overflow = self.overflow.lock().unwrap();

                if self.write_buffer.capacity() + overflow.len() + total_size
                    > WRITE_BUFFER_MAX_SIZE
                {
                    return Err(());
                }

                overflow.push(T::TYPE as u8);
                overflow.extend_from_slice(&(size as u32).to_le_bytes());
                bincode::serialize_into(&mut *overflow, message).unwrap();

                return Ok(());
            }

            match self.write_cursor.compare_exchange_weak(
                cursor,
                cursor + total_size,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(current) => cursor = current,
            }
        }

        *self.write_buffer.index(cursor) = T::TYPE as u8;
        cursor += 1;

        self.write_buffer
            .range(cursor..cursor + 4)
            .copy_from_slice(&(size as u32).to_le_bytes());
        cursor += 4;

        bincode::serialize_into(&mut self.write_buffer.range(cursor..cursor + size), message)
            .unwrap();

        Ok(())
    }

    fn next_message(&mut self) -> Option<(MessageType, &[u8])> {
        // XXX: Only less than, messages can be zero length
        if self.read_bytes - self.read_cursor < HEADER_SIZE {
            return None;
        }

        let message_type = self.read_buffer[self.read_cursor];
        if message_type >= MessageType::MAX as u8 {
            // Invalid message type, return invalid message so it disconnects.
            return Some((MessageType::MAX, &[]));
        }
        let message_type: MessageType = unsafe { std::mem::transmute(message_type) };

        let message_length = u32::from_le_bytes(
            self.read_buffer[self.read_cursor + 1..self.read_cursor + 5]
                .try_into()
                .unwrap(),
        ) as usize;

        if message_length > self.read_buffer.len() - HEADER_SIZE {
            // TODO: Would be useful for development to tell the client that it messed up.
            //
            // Messages larger than the read buffer will cause corruption of the stream, so we
            // return an invalid message that will fail to deserialize so that the client is
            // disconnected.
            return Some((message_type, &[]));
        } else if message_length > self.read_bytes - self.read_cursor - HEADER_SIZE {
            return None;
        }

        self.read_cursor += HEADER_SIZE;

        let message = &self.read_buffer[self.read_cursor..self.read_cursor + message_length];
        self.read_cursor += message_length;

        return Some((message_type, message));
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        self.socket.shutdown(std::net::Shutdown::Both).ok();
    }
}

// This represents a connection to a client that isn't ready to play.
// While the connection has not been authorized(username) it is considered illegitimate and will be
// disconnected if it does not provide one quickly. After it has been authorized, the client may
// request the game assets, and is given time to load them. When it sends a 'ClientReady'
// message, the connection is moved to the established connections.
struct UninitializedConnection {
    username: Option<String>,
    asset_download_progress: Option<usize>,
    connection: Option<Connection>,
}

impl UninitializedConnection {
    fn new(socket: TcpStream, address: SocketAddr) -> Self {
        Self {
            username: None,
            asset_download_progress: None,
            connection: Some(Connection::new(socket, address)),
        }
    }
}

/// Sent immediately after a client connects or disconnects
#[derive(Message)]
pub enum NetworkEvent {
    /// Provided for symmetry, prefer listening for `Added<Player>`
    Connected { entity: Entity },
    /// Signals that player connection has been disconnected. This event is only valid during
    /// PreUpdate of the tick after it is issued. After that the entity will have been despawned.
    Disconnected { entity: Entity },
}

// Separate out the server config so the connection code can be clearer
#[derive(SystemParam)]
struct ServerConfig<'w> {
    render_distance: Res<'w, MaxRenderDistance>,
    assets: Res<'w, Assets>,
    models: Res<'w, Models>,
    items: Res<'w, Items>,
    particle_effects: Res<'w, ParticleEffects>,
}

impl ServerConfig<'_> {
    fn to_message(&self) -> Vec<u8> {
        let server_config = messages::ServerConfig {
            assets_hash: self.assets.hash,
            block_ids: Blocks::get().ids().clone(),
            model_ids: self.models.ids().clone(),
            item_ids: self.items.ids().clone(),
            particle_effect_ids: self.particle_effects.ids().clone(),
            render_distance: self.render_distance.chunks,
        };

        let serialized_size = bincode::serialized_size(&server_config).unwrap() as u32;
        let mut serialized = Vec::new();
        serialized.push(MessageType::ServerConfig as u8);
        serialized.extend(serialized_size.to_le_bytes());
        serialized.extend(bincode::serialize(&server_config).unwrap());
        let compressed = zstd::bulk::compress(&serialized, 5).unwrap();
        let mut message = Vec::from((compressed.len() as u32).to_le_bytes());
        message.extend(compressed);

        message
    }
}

// TODO: Any error will cause disconnection. The player won't know what's wrong.
fn handle_new_connections(
    mut commands: Commands,
    assets: Res<Assets>,
    server_config: ServerConfig,
    mut server: ResMut<Server>,
    mut network_events: MessageWriter<NetworkEvent>,
    mut uninitialized_connections: Local<Vec<UninitializedConnection>>,
) {
    while let Ok((tcp_stream, socket_addr)) = server.listener.accept() {
        // TODO: This can probably panic but I don't know when
        tcp_stream
            .set_nodelay(true)
            .expect("Failed to set no_delay for a tcp connection");
        tcp_stream
            .set_nonblocking(true)
            .expect("Failed setting a tcp connection to non-blocking");

        uninitialized_connections.push(UninitializedConnection::new(tcp_stream, socket_addr));
    }

    uninitialized_connections.retain_mut(|uninitialized| {
        let connection = uninitialized.connection.as_mut().unwrap();
        if connection.read_from_socket().is_err() {
            return false;
        }

        if let Some(progress) = uninitialized.asset_download_progress {
            let sent = match connection.socket.write(&assets.asset_message[progress..]) {
                Ok(sent) => sent,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => 0,
                Err(_) => return false,
            };

            let new_progress = progress + sent;

            if new_progress == assets.asset_message.len() {
                uninitialized.asset_download_progress = None;
            } else {
                uninitialized.asset_download_progress = Some(new_progress);
            }
        }

        let Some((message_type, message)) = connection.next_message() else {
            return true;
        };

        if uninitialized.username.is_none() {
            if let Ok(identity) = bincode::deserialize::<messages::ClientIdentification>(message) {
                uninitialized.username = Some(identity.name.clone());
            } else {
                return false;
            }

            if connection
                .socket
                .write_all(&server_config.to_message())
                .is_err()
            {
                return false;
            }
        } else if message_type == MessageType::AssetRequest {
            // TODO: Need some way to bar clients from sending multiple requests. Some n attempts
            // per day.
            uninitialized.asset_download_progress = Some(0);
        } else if message_type == MessageType::ClientReady {
            let player_entity = commands
                .spawn(DefaultPlayerBundle::new(
                    uninitialized.username.take().unwrap(),
                ))
                .id();

            server
                .connections
                .insert(player_entity, uninitialized.connection.take().unwrap());

            network_events.write(NetworkEvent::Connected {
                entity: player_entity,
            });

            return false;
        } else {
            return false;
        }

        return true;
    });
}

fn disconnect_players(mut network_events: MessageWriter<NetworkEvent>, server: ResMut<Server>) {
    // Can't split borrows when behind a ResMut
    let server = server.into_inner();

    for connection_entity in server.to_disconnect.lock().unwrap().drain() {
        if server.connections.remove(&connection_entity).is_some() {
            network_events.write(NetworkEvent::Disconnected {
                entity: connection_entity,
            });
        }
    }
}

fn remove_disconnected_player_entities(
    mut commands: Commands,
    mut network_events: MessageReader<NetworkEvent>,
) {
    for network_event in network_events.read() {
        if let NetworkEvent::Disconnected { entity } = network_event {
            commands.entity(*entity).despawn();
        }
    }
}

fn log_connections(
    server: Res<Server>,
    player_query: Query<&Player>,
    mut network_events: MessageReader<NetworkEvent>,
) {
    for network_event in network_events.read() {
        match network_event {
            NetworkEvent::Connected { entity } => {
                let player = player_query.get(*entity).unwrap();
                let connection = server.connections.get(entity).unwrap();
                info!(
                    "Player connected, ip: {}, username: {}",
                    connection.address, &player.username
                );
            }
            NetworkEvent::Disconnected { entity } => {
                let player = player_query.get(*entity).unwrap();
                info!("Player disconnected, username: {}", &player.username);
            }
        }
    }
}

/// A message received from a client
#[derive(Message, Deref, Debug)]
pub struct NetworkMessage<T: fmc_protocol::ServerBound> {
    pub player_entity: Entity,
    #[deref]
    pub message: T,
}

// All ServerBound message events that are valid after connection initialization
#[derive(SystemParam)]
struct MessageWriters<'w> {
    left_click: MessageWriter<'w, NetworkMessage<messages::LeftClick>>,
    right_click: MessageWriter<'w, NetworkMessage<messages::RightClick>>,
    render_distance: MessageWriter<'w, NetworkMessage<messages::RenderDistance>>,
    player_camera_rotation: MessageWriter<'w, NetworkMessage<messages::PlayerCameraRotation>>,
    player_position: MessageWriter<'w, NetworkMessage<messages::PlayerPosition>>,
    interface_equip_item: MessageWriter<'w, NetworkMessage<messages::InterfaceEquipItem>>,
    interface_interaction: MessageWriter<'w, NetworkMessage<messages::InterfaceInteraction>>,
    interface_text_input: MessageWriter<'w, NetworkMessage<messages::InterfaceTextInput>>,
    gui_setting: MessageWriter<'w, NetworkMessage<messages::GuiSetting>>,
}

fn read_messages(server: ResMut<Server>, mut event_writers: MessageWriters) {
    let server = server.into_inner();

    // During the last tick the message/compression buffer might have grown. This growth is for surge
    // protection and shouldn't be permanent. So we reset it each read.
    server.compression_buffer.resize(WRITE_BUFFER_SIZE, 0);
    server.compression_buffer.shrink_to_fit();

    for (entity, connection) in server.connections.iter_mut() {
        if connection.read_from_socket().is_err() {
            server.to_disconnect.insert(*entity);
            continue;
        };

        while let Some((message_type, message_data)) = connection.next_message() {
            macro_rules! receive {
                ($writer:ident) => {
                    if let Ok(message) = bincode::deserialize(message_data) {
                        event_writers.$writer.write(NetworkMessage {
                            player_entity: *entity,
                            message,
                        });
                    } else {
                        server.to_disconnect.insert(*entity);
                        error!(
                            "Received {:?} from {}, but the message could not be deserialized, disconnecting client.",
                            message_type, connection.address
                        );
                        break;
                    }
                };
            }

            match message_type {
                MessageType::LeftClick => receive!(left_click),
                MessageType::RightClick => receive!(right_click),
                MessageType::RenderDistance => receive!(render_distance),
                MessageType::PlayerCameraRotation => receive!(player_camera_rotation),
                MessageType::PlayerPosition => receive!(player_position),
                MessageType::InterfaceEquipItem => receive!(interface_equip_item),
                MessageType::InterfaceInteraction => receive!(interface_interaction),
                MessageType::InterfaceTextInput => receive!(interface_text_input),
                MessageType::GuiSetting => receive!(gui_setting),
                _ => {
                    server.to_disconnect.insert(*entity);
                    error!(
                        "Received invalid message type {:?} from {}, disconnecting client.",
                        message_type, connection.address
                    );
                    break;
                }
            }
        }
    }
    server.safe.store(true, Ordering::Relaxed);
}

fn send_messages(server: ResMut<Server>) {
    let server = server.into_inner();
    for (entity, connection) in server.connections.iter_mut() {
        // Encode what has been written to the write buffer
        let len = connection.write_cursor.swap(0, Ordering::Relaxed);
        let overflow = std::mem::take(&mut *connection.overflow.lock().unwrap());

        if len == 0 && overflow.is_empty() {
            continue;
        }

        let total_len = len + overflow.len();
        server.compression_buffer.resize(
            COMPRESSION_HEADER + zstd::zstd_safe::compress_bound(total_len),
            0,
        );

        // Reserve first bytes to store the compression header
        let mut encoder =
            zstd::Encoder::new(&mut server.compression_buffer[COMPRESSION_HEADER..], 5).unwrap();

        encoder
            .set_pledged_src_size(Some(total_len as u64))
            .unwrap();
        encoder
            .write_all(connection.write_buffer.range_to(..len))
            .unwrap();
        encoder.write_all(&overflow).unwrap();

        // Use how much was left of the compression_buffer to determine length of what was written.
        let remaining = encoder.finish().unwrap().len();
        let encoded_len = (server.compression_buffer.len() - remaining - COMPRESSION_HEADER) as u32;
        server.compression_buffer[..COMPRESSION_HEADER].copy_from_slice(&encoded_len.to_le_bytes());

        let frame = &server.compression_buffer[..COMPRESSION_HEADER + encoded_len as usize];
        match connection.socket.write_all(frame) {
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                // The kernel buffer is full, probably because of a slow connection. The buffer can
                // hold a couple of megabytes so it will optimistically never occur, but if it does the
                // client has to be disconnected as continuing would cause loss of data when the
                // write buffer is rotated.
                if server.to_disconnect.insert(*entity) {
                    error!(
                        "Connection to player too slow, write buffer at capacity, disconnecting player."
                    );
                }
            }
            Err(e) => {
                if server.to_disconnect.insert(*entity) {
                    error!("Encountered error while sending messages to player: {}", e);
                }
            }
            Ok(()) => (),
        }
    }
    server.safe.store(false, Ordering::Relaxed);
}
