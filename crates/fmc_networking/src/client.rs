use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::Arc,
};

use bevy::prelude::*;
use dashmap::DashMap;
use derive_more::Display;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpStream, ToSocketAddrs},
    runtime::Runtime,
    sync::mpsc::{unbounded_channel, UnboundedSender},
    task::JoinHandle,
};

use crate::{
    error::ClientNetworkError,
    network_message::{ClientBound, NetworkMessage, ServerBound},
    ClientNetworkEvent, ConnectionId, NetworkData, NetworkPacket, NetworkSettings, SyncChannel,
};

#[derive(Display)]
#[display(fmt = "Server connection to {}", peer_addr)]
struct ServerConnection {
    peer_addr: SocketAddr,
    receive_task: JoinHandle<()>,
    send_task: JoinHandle<()>,
    send_message: UnboundedSender<NetworkPacket>,
}

impl ServerConnection {
    fn stop(self) {
        self.receive_task.abort();
        self.send_task.abort();
    }
}

/// An instance of a [`NetworkClient`] is used to connect to a remote server
/// using [`NetworkClient::connect`]
#[derive(Resource)]
pub struct NetworkClient {
    runtime: Runtime,
    server_connection: Option<ServerConnection>,
    recv_message_map: Arc<DashMap<&'static str, Vec<Box<dyn NetworkMessage>>>>,
    network_events: SyncChannel<ClientNetworkEvent>,
    connection_events: SyncChannel<(TcpStream, SocketAddr)>,
}

impl std::fmt::Debug for NetworkClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(conn) = self.server_connection.as_ref() {
            write!(f, "NetworkClient [Connected to {}]", conn.peer_addr)?;
        } else {
            write!(f, "NetworkClient [Not Connected]")?;
        }

        Ok(())
    }
}

impl NetworkClient {
    pub(crate) fn new() -> NetworkClient {
        NetworkClient {
            runtime: tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("Could not build tokio runtime"),
            server_connection: None,
            recv_message_map: Arc::new(DashMap::new()),
            network_events: SyncChannel::new(),
            connection_events: SyncChannel::new(),
        }
    }

    /// Connect to a remote server
    pub fn connect(&mut self, addr: impl ToSocketAddrs + Send + 'static) {
        debug!("Starting connection");

        if self.is_connected() {
            panic!("The client is already connected")
        }

        let network_events_sender = self.network_events.sender.clone();
        let connection_event_sender = self.connection_events.sender.clone();

        self.runtime.spawn(async move {
            let stream = match TcpStream::connect(addr).await {
                Ok(stream) => stream,
                Err(error) => {
                    match network_events_sender.send(ClientNetworkEvent::Error(
                        ClientNetworkError::ConnectionRefused(error),
                    )) {
                        Ok(_) => (),
                        Err(err) => {
                            error!("Could not send error event: {}", err);
                        }
                    }

                    return;
                }
            };

            let addr = stream
                .peer_addr()
                .expect("Could not fetch peer_addr of existing stream");

            match connection_event_sender.send((stream, addr)) {
                Ok(_) => (),
                Err(err) => {
                    error!("Could not initiate connection: {}", err);
                }
            }

            debug!("Connected to: {:?}", addr);
        });
    }

    /// Initiate a disconnect, it will not disconnect before the next update cycle.
    /// The message is shown to the player.
    #[track_caller]
    pub fn disconnect<T: AsRef<str>>(&self, message: T) {
        // Log all disconnects that provide a message as errors (even though some aren't)
        if message.as_ref().len() > 0 {
            error!("{}: {}", std::panic::Location::caller(), message.as_ref());
        }

        self.network_events
            .sender
            .send(ClientNetworkEvent::Disconnected(
                message.as_ref().to_owned(),
            ))
            .unwrap();
    }

    /// Send a message to the connected server.
    /// The message is voided if the client is/has been disconnected.
    pub fn send_message<T: ServerBound>(&self, message: T) {
        debug!("Sending message to server");
        let server_connection = match self.server_connection.as_ref() {
            Some(server) => server,
            None => return,
        };

        let packet = NetworkPacket {
            kind: String::from(T::NAME),
            data: Box::new(message),
        };

        // XXX: If the receiver half of 'self.send_message' has been closed, the server
        // unexpectedly disconnected. This code is the only thing that will discover it.
        if !server_connection.send_message.send(packet).is_ok() {
            self.network_events
                .sender
                .send(ClientNetworkEvent::Error(
                    ClientNetworkError::ConnectionLost,
                ))
                .unwrap();
        };
    }

    pub fn connection_id(&self) -> ConnectionId {
        ConnectionId {
            entity: Entity::PLACEHOLDER,
            addr: self
                .server_connection
                .as_ref()
                .map(|conn| conn.peer_addr)
                .unwrap_or_else(|| SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0)),
        }
    }

    /// Returns true if the client has an established connection
    ///
    /// # Note
    /// This may return true even if the connection has already been broken on the server side.
    pub fn is_connected(&self) -> bool {
        self.server_connection.is_some()
    }
}

/// A utility trait on [`AppBuilder`] to easily register [`ClientMessage`]s
pub trait AppNetworkClientMessage {
    /// Register a client message type
    ///
    /// ## Details
    /// This will:
    /// - Add a new event type of [`NetworkData<T>`]
    /// - Register the type for transformation over the wire
    /// - Internal bookkeeping
    fn listen_for_client_message<T: ClientBound>(&mut self) -> &mut Self;
}

impl AppNetworkClientMessage for App {
    fn listen_for_client_message<T: ClientBound>(&mut self) -> &mut Self {
        let client = self.world.get_resource::<NetworkClient>().expect("Could not find `NetworkClient`. Be sure to include the `ClientPlugin` before listening for client messages.");

        debug!("Registered a new ClientMessage: {}", T::NAME);

        assert!(
            !client.recv_message_map.contains_key(T::NAME),
            "Duplicate registration of ClientMessage: {}",
            T::NAME
        );
        client.recv_message_map.insert(T::NAME, Vec::new());

        self.add_event::<NetworkData<T>>();
        self.add_systems(PreUpdate, register_client_message::<T>)
    }
}

fn register_client_message<T>(
    net_res: ResMut<NetworkClient>,
    mut events: EventWriter<NetworkData<T>>,
) where
    T: ClientBound,
{
    let mut messages = match net_res.recv_message_map.get_mut(T::NAME) {
        Some(messages) => messages,
        None => return,
    };

    events.send_batch(
        messages
            .drain(..)
            .flat_map(|msg| msg.downcast())
            .map(|msg| NetworkData::new(net_res.connection_id(), *msg)),
    );
}

pub fn handle_connection_event(
    mut net_res: ResMut<NetworkClient>,
    mut events: EventWriter<ClientNetworkEvent>,
) {
    let (connection, peer_addr) = match net_res.connection_events.receiver.try_recv() {
        Ok(event) => event,
        Err(_) => {
            return;
        }
    };

    let (read_socket, send_socket) = connection.into_split();
    let recv_message_map = net_res.recv_message_map.clone();
    let (send_message, recv_message) = unbounded_channel();
    let send_settings = NetworkSettings::default();

    net_res.server_connection = Some(ServerConnection {
        peer_addr,
        send_task: net_res.runtime.spawn(async move {
            let mut recv_message = recv_message;
            let mut send_socket = send_socket;
            let mut buffer: Vec<u8> = vec![0; send_settings.max_packet_length];

            while let Some(message) = recv_message.recv().await {
                let size = match bincode::serialized_size(&message) {
                    Ok(size) => size as usize,
                    Err(err) => {
                        error!(
                            "Could not get the size of the packet {:?}: {}",
                            message, err
                        );
                        continue;
                    }
                };

                match bincode::serialize_into(&mut buffer[0..size], &message) {
                    Ok(_) => (),
                    Err(err) => {
                        error!(
                            "Coult not serialize packet into buffer {:?}: {}",
                            message, err
                        );
                        continue;
                    }
                };

                debug!("Sending a new message of size: {}", size);

                match send_socket.write_u32(size as u32).await {
                    Ok(_) => (),
                    Err(err) => {
                        error!("Could not send packet length: {:?}: {}", size, err);
                        break;
                    }
                }

                trace!("Sending the content of the message");

                match send_socket.write_all(&buffer[0..size]).await {
                    Ok(_) => (),
                    Err(err) => {
                        error!("Could not send packet: {:?}: {}", message, err);
                        return;
                    }
                }

                trace!("Succesfully sent message");
            }
        }),
        receive_task: net_res.runtime.spawn(async move {
            let mut read_socket = read_socket;
            let network_settings = NetworkSettings::default();
            let recv_message_map = recv_message_map;

            let mut buffer: Vec<u8> = vec![0; network_settings.max_packet_length];
            loop {
                let length = match read_socket.read_u32().await {
                    Ok(len) => len as usize,
                    Err(err) => {
                        error!(
                            "Encountered error while fetching length [{}]: {}",
                            peer_addr, err
                        );
                        break;
                    }
                };

                if length > network_settings.max_packet_length {
                    error!(
                        "Received too large packet from [{}]: {} > {}",
                        peer_addr, length, network_settings.max_packet_length
                    );
                    break;
                }

                match read_socket.read_exact(&mut buffer[..length]).await {
                    Ok(_) => (),
                    Err(err) => {
                        error!(
                            "Encountered error while fetching stream of length {} [{}]: {}",
                            length, peer_addr, err
                        );
                        break;
                    }
                }

                let packet: NetworkPacket = match bincode::deserialize(&buffer[..length]) {
                    Ok(packet) => packet,
                    Err(err) => {
                        error!(
                            "Failed to decode network packet from [{}]: {}",
                            peer_addr, err
                        );
                        break;
                    }
                };

                match recv_message_map.get_mut(&packet.kind[..]) {
                    Some(mut packets) => packets.push(packet.data),
                    None => {
                        error!(
                            "Could not find existing entries for message kinds: {:?}",
                            packet
                        );
                    }
                }
                debug!("Received message from: {}", peer_addr);
            }
        }),
        send_message,
    });

    events.send(ClientNetworkEvent::Connected);
}

pub fn handle_client_network_events(
    mut net: ResMut<NetworkClient>,
    mut server_disconnect_events: EventReader<NetworkData<crate::messages::Disconnect>>,
    mut client_network_events: EventWriter<ClientNetworkEvent>,
) {
    for event in server_disconnect_events.read() {
        net.network_events
            .sender
            .send(ClientNetworkEvent::Disconnected(event.message.to_owned()))
            .unwrap();
        break;
    }

    for event in net.network_events.receiver.try_iter() {
        match event {
            ClientNetworkEvent::Error(_) | ClientNetworkEvent::Disconnected(_) => {
                if let Some(connection) = net.server_connection.take() {
                    connection.stop();
                }
                client_network_events.send(event);
                // There might be many errors when something bad happens, so just send the first
                // one. The others will be cleared on event buffer rotation at the end of the
                // update.
                return;
            }
            ClientNetworkEvent::Connected => client_network_events.send(event),
        };
    }
}
