use std::{
    collections::{HashMap, HashSet},
    io::{Read, Write},
    net::{SocketAddr, TcpStream},
    ops::{Range, RangeFrom, RangeTo},
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Mutex,
    },
};

use bevy::{ecs::system::SystemParam, utils::syncunsafecell::SyncUnsafeCell};
use fmc_protocol::{messages, ClientBound, MessageType};
use serde::Serialize;

use crate::{
    assets::Assets,
    blocks::Blocks,
    items::Items,
    models::Models,
    players::{DefaultPlayerBundle, Player},
    prelude::*,
    world::RenderDistance,
};

// Size of each connection's read/write buffer
const MESSAGE_BUFFER_SIZE: usize = 1024 * 1024;
const MESSAGE_BUFFER_MAX_SIZE: usize = 32 * 1024 * 1024;
// MessageType (1 byte) + message length (4 bytes)
const HEADER_SIZE: usize = 5;
// message length (4 bytes)
const COMPRESSION_HEADER: usize = 4;

pub struct ServerPlugin;
impl Plugin for ServerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, server_setup)
            .add_event::<NetworkEvent>()
            .add_event::<NetworkMessage<messages::LeftClick>>()
            .add_event::<NetworkMessage<messages::RightClick>>()
            .add_event::<NetworkMessage<messages::RenderDistance>>()
            .add_event::<NetworkMessage<messages::PlayerCameraRotation>>()
            .add_event::<NetworkMessage<messages::PlayerPosition>>()
            .add_event::<NetworkMessage<messages::InterfaceEquipItem>>()
            .add_event::<NetworkMessage<messages::InterfaceInteraction>>()
            .add_event::<NetworkMessage<messages::InterfaceTextInput>>()
            .add_systems(First, read_messages)
            .add_systems(
                PreUpdate,
                (
                    // XXX: Remember that new connnections should always be added after messages are
                    // read so that the server has one tick to register components to the player it
                    // needs to handle messages.
                    handle_new_connections,
                    log_connections,
                ),
            )
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
    let socket_address: SocketAddr = "127.0.0.1:42069".parse().unwrap();

    let listener = std::net::TcpListener::bind(socket_address).unwrap();
    listener.set_nonblocking(true).unwrap();

    let server = Server {
        listener,
        connections: HashMap::new(),
        to_disconnect: Disconnections::default(),
        compression_buffer: vec![0; MESSAGE_BUFFER_SIZE],
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
        if self.safe.load(Ordering::Relaxed) != true {
            panic!();
        }

        for connection_entity in connection_entities.into_iter().cloned() {
            let Some(connection) = self.connections.get(&connection_entity) else {
                // TODO: Server isn't supposed to have access to removed connection entities?
                // Continue added to deal with later
                continue;
            };

            if connection.write_message(&message).is_err() {
                if self.disconnect(connection_entity) {
                    error!(
                        "Failed to send message, the player's message buffer is at capacity. Server is \
                        sending too much, or the connection is too slow. Disconnecting to prevent the \
                        client from being left in an unsynchronised state."
                    );
                };
            };
        }
    }

    #[track_caller]
    pub fn broadcast<'a, T: ClientBound + Serialize>(&self, message: T) {
        self.send_many(self.connections.keys(), message);
    }

    pub fn disconnect(&self, connection_entity: Entity) -> bool {
        self.to_disconnect.insert(connection_entity)
    }
}

// TODO: There's no reason to share between read and write buffers. The read buffer can be ~1kb
// large and it will likely be more than enough.
//
// To save memory each connection gets only one buffer allocation for read/writes.
// Since it is shared, we need to be careful when we read and write as they cannot be allowed to
// happen at the same time.
//
// 1. The buffer is only used to read from the socket in the 'First' schedule
// 2. The buffer is only written to in the 'PreUpdate', 'Update' or 'PostUpdate' schedules.
// 3. The messages are written to socket in the 'Last' schedule
struct MessageBuffer(SyncUnsafeCell<Vec<u8>>);

impl MessageBuffer {
    fn new() -> Self {
        Self(SyncUnsafeCell::new(vec![0; MESSAGE_BUFFER_SIZE]))
    }

    fn capacity(&self) -> usize {
        unsafe { (*(self.0.get())).capacity() }
    }

    fn shrink(&self, amount: usize) {
        unsafe {
            let buf = &mut *(self.0.get());
            buf.resize(amount, 0);
            buf.shrink_to_fit();
        }
    }

    fn grow(&self, amount: usize) {
        unsafe {
            let buf = &mut *(self.0.get());
            let new_cap = buf.capacity() * 2;
            buf.resize(new_cap.max(amount), 0);
        }
    }

    // TODO: This could be implemented with SliceIndex I think, but it requires an unstable flag
    // and I somewhat prefer having all the methods, makes you aware you are doing something
    // dangerous.
    fn index(&self, index: usize) -> &mut u8 {
        unsafe { &mut (*self.0.get())[index] }
    }

    fn range(&self, index: Range<usize>) -> &mut [u8] {
        unsafe { &mut (*self.0.get())[index] }
    }

    #[track_caller]
    fn range_to(&self, index: RangeTo<usize>) -> &mut [u8] {
        unsafe { &mut (*self.0.get())[index] }
    }

    fn range_from(&self, index: RangeFrom<usize>) -> &mut [u8] {
        unsafe { &mut (*self.0.get())[index] }
    }
}

struct Connection {
    socket: TcpStream,
    address: SocketAddr,
    message_buffer: MessageBuffer,
    read_cursor: usize,
    read_bytes: usize,
    write_cursor: AtomicUsize,
    is_growing: AtomicBool,
    // (length, data) of a partially received message. When the message buffer is used for writing,
    // partially read messages are stored here, and then moved back into the buffer when it's time
    // to read again.
    partially_read_message: (usize, [u8; 1024]),
}

impl Connection {
    fn new(socket: TcpStream, address: SocketAddr) -> Self {
        Self {
            socket,
            address,
            message_buffer: MessageBuffer::new(),
            read_cursor: 0,
            read_bytes: 0,
            write_cursor: AtomicUsize::new(0),
            is_growing: AtomicBool::new(false),
            partially_read_message: (0, [0; 1024]),
        }
    }

    fn read_from_socket(&mut self) -> std::io::Result<usize> {
        self.load_partial_message();
        match self
            .socket
            .read(&mut self.message_buffer.range_from(self.read_bytes..))
        {
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
        let mut cursor = self
            .write_cursor
            .fetch_add(size + HEADER_SIZE, Ordering::Relaxed);

        // TODO: It's very likely that reading the messsage buffer len while it is being grown is
        // bad.
        //
        // This is for surge protection, it should happen rarely. The MESSAGE_BUFFER_SIZE should be
        // set to a value that covers 99% of operation. Happens for example when a player gets
        // spawned at a spot where all the chunks are already loaded, and get sent at the same
        // time.
        // The allocated memory is freed again when reading messages the next tick.
        let final_len = cursor + size + HEADER_SIZE;
        if final_len > self.message_buffer.capacity() {
            if self.message_buffer.capacity() >= MESSAGE_BUFFER_MAX_SIZE {
                return Err(());
            }

            // Another write function may already be in the process of growing the buffer.
            let currently_growing = self
                .is_growing
                .compare_exchange(false, true, Ordering::Relaxed, Ordering::Relaxed)
                .is_err();
            if currently_growing {
                // If there is, loop until it finishes growing
                while cursor + size + HEADER_SIZE > self.message_buffer.capacity() {}
            } else {
                // The first capacity check is done before the atomic exchange. In this timespan
                // another write function might finish growing it, so we check again so it doesn't
                // grow it again.
                if final_len > self.message_buffer.capacity() {
                    self.message_buffer.grow(size + HEADER_SIZE);
                }
                self.is_growing.store(false, Ordering::Relaxed);
            }
        }

        *self.message_buffer.index(cursor) = T::TYPE as u8;
        cursor += 1;

        self.message_buffer
            .range(cursor..cursor + 4)
            .copy_from_slice(&(size as u32).to_le_bytes());
        cursor += 4;

        bincode::serialize_into(
            &mut self.message_buffer.range(cursor..cursor + size),
            message,
        )
        .unwrap();

        Ok(())
    }

    fn next_message(&mut self) -> Option<(MessageType, &[u8])> {
        // XXX: Only less than, messages can be zero length
        if self.read_bytes - self.read_cursor < HEADER_SIZE {
            self.save_partial_message();
            return None;
        }

        let message_type = self.message_buffer.index(self.read_cursor);
        if *message_type >= MessageType::MAX as u8 {
            // Invalid message type, return invalid message so it disconnects.
            return Some((MessageType::MAX, &[]));
        }
        let message_type: MessageType = unsafe { std::mem::transmute(*message_type) };

        let message_length = u32::from_le_bytes(
            self.message_buffer
                .range(self.read_cursor + 1..self.read_cursor + 5)
                .try_into()
                .unwrap(),
        ) as usize;

        if message_length > self.partially_read_message.1.len() - HEADER_SIZE {
            // TODO: Would be useful for development to tell the client that it messed up.
            //
            // Messages larger than the partial buffer will cause corruption of the stream, so we
            // return an invalid message that will fail to deserialize so that the client is
            // disconnected.
            return Some((message_type, &[]));
        } else if self.read_bytes - self.read_cursor + HEADER_SIZE < message_length {
            self.save_partial_message();
            return None;
        }

        self.read_cursor += HEADER_SIZE;

        let message = self
            .message_buffer
            .range(self.read_cursor..self.read_cursor + message_length);
        self.read_cursor += message_length;
        if self.read_cursor > self.read_bytes {
            panic!();
        }

        return Some((message_type, message));
    }

    fn save_partial_message(&mut self) {
        let remaining = &self.message_buffer.range(self.read_cursor..self.read_bytes);
        let length = remaining.len();
        self.partially_read_message.0 = length;
        self.partially_read_message.1[..length].copy_from_slice(remaining);
    }

    fn load_partial_message(&mut self) {
        let length = self.partially_read_message.0;
        self.message_buffer
            .range_to(..length)
            .copy_from_slice(&self.partially_read_message.1[..length]);
        self.read_cursor = 0;
        self.read_bytes = length;
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

#[derive(Event)]
pub enum NetworkEvent {
    // Provided for symmetry, prefer listening for Added<Player>
    Connected { entity: Entity },
    // Signals that player connection has been disconnected. This event is only valid during
    // PreUpdate of the tick after it is issued. After that the entity will have been despawned.
    Disconnected { entity: Entity },
}

// Separate out the server config so the connection code can be clearer
#[derive(SystemParam)]
struct ServerConfig<'w> {
    render_distance: Res<'w, RenderDistance>,
    assets: Res<'w, Assets>,
    models: Res<'w, Models>,
    items: Res<'w, Items>,
}

impl ServerConfig<'_> {
    fn to_message(&self) -> Vec<u8> {
        let server_config = messages::ServerConfig {
            assets_hash: self.assets.hash,
            block_ids: Blocks::get().asset_ids(),
            model_ids: self.models.asset_ids(),
            item_ids: self.items.asset_ids(),
            render_distance: self.render_distance.chunks,
        };

        let serialized_size = bincode::serialized_size(&server_config).unwrap() as u32;
        let mut serialized = Vec::new();
        serialized.push(MessageType::ServerConfig as u8);
        serialized.extend(serialized_size.to_le_bytes());
        serialized.extend(bincode::serialize(&server_config).unwrap());
        let compressed = zstd::encode_all(&serialized[..], 5).unwrap();
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
    mut network_events: EventWriter<NetworkEvent>,
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
            let Ok(sent) = connection.socket.write(&assets.asset_message[progress..]) else {
                return false;
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
                .write(&server_config.to_message())
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

            // More messages might have arrived, we'll be able to handle them next tick.
            connection.save_partial_message();

            server
                .connections
                .insert(player_entity, uninitialized.connection.take().unwrap());

            network_events.send(NetworkEvent::Connected {
                entity: player_entity,
            });

            return false;
        } else {
            return false;
        }

        return true;
    });
}

// This drops the connection, but does not despawn the entity. Despawning is delayed until
// PreUpdate to give the application time to save the player data.
fn disconnect_players(mut network_events: EventWriter<NetworkEvent>, server: ResMut<Server>) {
    // Can't split borrows when behind a ResMut
    let server = server.into_inner();

    for connection_entity in server.to_disconnect.lock().unwrap().drain() {
        if server.connections.remove(&connection_entity).is_some() {
            network_events.send(NetworkEvent::Disconnected {
                entity: connection_entity,
            });
        }
    }
}

fn remove_disconnected_player_entities(
    mut commands: Commands,
    mut network_events: EventReader<NetworkEvent>,
) {
    for network_event in network_events.read() {
        if let NetworkEvent::Disconnected { entity } = network_event {
            commands.entity(*entity).despawn_recursive();
        }
    }
}

fn log_connections(
    server: Res<Server>,
    player_query: Query<&Player>,
    mut network_events: EventReader<NetworkEvent>,
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

#[derive(Event, Deref, Debug)]
pub struct NetworkMessage<T> {
    pub player_entity: Entity,
    #[deref]
    pub message: T,
}

// All ServerBound message events that are valid after connection initialization
#[derive(SystemParam)]
struct EventWriters<'w> {
    left_click: EventWriter<'w, NetworkMessage<messages::LeftClick>>,
    right_click: EventWriter<'w, NetworkMessage<messages::RightClick>>,
    render_distance: EventWriter<'w, NetworkMessage<messages::RenderDistance>>,
    player_camera_rotation: EventWriter<'w, NetworkMessage<messages::PlayerCameraRotation>>,
    player_position: EventWriter<'w, NetworkMessage<messages::PlayerPosition>>,
    interface_equip_item: EventWriter<'w, NetworkMessage<messages::InterfaceEquipItem>>,
    interface_interaction: EventWriter<'w, NetworkMessage<messages::InterfaceInteraction>>,
    interface_text_input: EventWriter<'w, NetworkMessage<messages::InterfaceTextInput>>,
}

fn read_messages(server: ResMut<Server>, mut event_writers: EventWriters) {
    let server = server.into_inner();

    // During the last tick the message/compression buffer might have grown. This growth is for surge
    // protection and shouldn't be permanent. So we reset it each read.
    server.compression_buffer.resize(MESSAGE_BUFFER_SIZE, 0);
    server.compression_buffer.shrink_to_fit();

    for (entity, connection) in server.connections.iter_mut() {
        connection.message_buffer.shrink(MESSAGE_BUFFER_SIZE);
        if connection.read_from_socket().is_err() {
            server.to_disconnect.insert(*entity);
            continue;
        };

        while let Some((message_type, message_data)) = connection.next_message() {
            match message_type {
                MessageType::LeftClick => {
                    if let Ok(message) = bincode::deserialize(message_data) {
                        event_writers.left_click.send(NetworkMessage {
                            player_entity: *entity,
                            message,
                        });
                    } else {
                        server.to_disconnect.insert(*entity);
                        error!("Received {:?} from {}, but the message could not be deserialized, disconnecting client.",
                            message_type, connection.address);
                        break;
                    }
                }
                MessageType::RightClick => {
                    if let Ok(message) = bincode::deserialize(message_data) {
                        event_writers.right_click.send(NetworkMessage {
                            player_entity: *entity,
                            message,
                        });
                    } else {
                        server.to_disconnect.insert(*entity);
                        error!("Received {:?} from {}, but the message could not be deserialized, disconnecting client.",
                            message_type, connection.address);
                        break;
                    }
                }
                MessageType::RenderDistance => {
                    if let Ok(message) = bincode::deserialize(message_data) {
                        event_writers.render_distance.send(NetworkMessage {
                            player_entity: *entity,
                            message,
                        });
                    } else {
                        server.to_disconnect.insert(*entity);
                        error!("Received {:?} from {}, but the message could not be deserialized, disconnecting client.",
                            message_type, connection.address);
                        break;
                    }
                }
                MessageType::PlayerCameraRotation => {
                    if let Ok(message) = bincode::deserialize(message_data) {
                        event_writers.player_camera_rotation.send(NetworkMessage {
                            player_entity: *entity,
                            message,
                        });
                    } else {
                        server.to_disconnect.insert(*entity);
                        error!("Received {:?} from {}, but the message could not be deserialized, disconnecting client.",
                            message_type, connection.address);
                        break;
                    }
                }
                MessageType::PlayerPosition => {
                    if let Ok(message) = bincode::deserialize(message_data) {
                        event_writers.player_position.send(NetworkMessage {
                            player_entity: *entity,
                            message,
                        });
                    } else {
                        server.to_disconnect.insert(*entity);
                        error!("Received {:?} from {}, but the message could not be deserialized, disconnecting client.",
                            message_type, connection.address);
                        break;
                    }
                }
                MessageType::InterfaceEquipItem => {
                    if let Ok(message) = bincode::deserialize(message_data) {
                        event_writers.interface_equip_item.send(NetworkMessage {
                            player_entity: *entity,
                            message,
                        });
                    } else {
                        server.to_disconnect.insert(*entity);
                        error!("Received {:?} from {}, but the message could not be deserialized, disconnecting client.",
                            message_type, connection.address);
                        break;
                    }
                }
                MessageType::InterfaceInteraction => {
                    if let Ok(message) = bincode::deserialize(message_data) {
                        event_writers.interface_interaction.send(NetworkMessage {
                            player_entity: *entity,
                            message,
                        });
                    } else {
                        server.to_disconnect.insert(*entity);
                        error!("Received {:?} from {}, but the message could not be deserialized, disconnecting client.",
                            message_type, connection.address);
                        break;
                    }
                }
                MessageType::InterfaceTextInput => {
                    if let Ok(message) = bincode::deserialize(message_data) {
                        event_writers.interface_text_input.send(NetworkMessage {
                            player_entity: *entity,
                            message,
                        });
                    } else {
                        server.to_disconnect.insert(*entity);
                        error!("Received {:?} from {}, but the message could not be deserialized, disconnecting client.",
                            message_type, connection.address);
                        break;
                    }
                }
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
        // Encode what has been written to the message buffer
        let len = connection.write_cursor.swap(0, Ordering::Relaxed);

        if len == 0 {
            continue;
        }

        // The message buffer might have grown so the compression buffer must also
        server
            .compression_buffer
            .resize(connection.message_buffer.capacity(), 0);

        // Reserve first bytes to store the compression header
        let mut encoder =
            zstd::Encoder::new(&mut server.compression_buffer[COMPRESSION_HEADER..], 5).unwrap();

        encoder.set_pledged_src_size(Some(len as u64)).unwrap();
        std::io::copy(
            &mut connection.message_buffer.range_to(..len).as_ref(),
            &mut encoder,
        )
        .unwrap();

        // Use how much was left of the compression_buffer to determine length of what was written.
        let remaining = encoder.finish().unwrap().len();
        let encoded_len = (server.compression_buffer.len() - remaining - 4) as u32;
        server.compression_buffer[..COMPRESSION_HEADER].copy_from_slice(&encoded_len.to_le_bytes());

        match connection
            .socket
            .write(&server.compression_buffer[..4 + encoded_len as usize])
        {
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                // The kernel buffer is full, probably because of a slow connection. The buffer can
                // hold a couple of megabytes so it will optimistically never occur, but if it does the
                // client has to be disconnected as continuing would cause loss of data when the
                // message buffer is rotated.
                if server.to_disconnect.insert(*entity) {
                    error!("Connection to player too slow, write buffer at capacity, disconnecting player.");
                }
            }
            Err(e) => {
                if server.to_disconnect.insert(*entity) {
                    error!("Encountered error while sending messages to player: {}", e);
                }
            }
            Ok(_) => (),
        }
    }
    server.safe.store(false, Ordering::Relaxed);
}
