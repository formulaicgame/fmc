use bevy::{math::DVec3, prelude::*};
use fmc_networking_derive::{ClientBound, NetworkMessage, ServerBound};
use serde::{Deserialize, Serialize};

/// Configure the player's aabb
#[derive(NetworkMessage, ClientBound, Serialize, Deserialize, Debug, Clone)]
pub struct PlayerAabb {
    pub center: Vec3,
    pub half_extents: Vec3,
}

// TODO: This doesn't need to be f64, the server can just convert it. The velocity is also only
// sent for convenience, it's slightly hard to compute server side, and since I haven't thought
// about validation it might as well abuse it.
/// A player's position. Used by client to report its position or for the server to dictate.
#[derive(NetworkMessage, ClientBound, ServerBound, Serialize, Deserialize, Debug, Clone)]
pub struct PlayerPosition {
    /// Position of the player.
    pub position: DVec3,
    /// Velocity of the player
    pub velocity: DVec3,
}

/// The position the server wants to place the player's camera in.
#[derive(NetworkMessage, ClientBound, Serialize, Deserialize, Debug, Clone)]
pub struct PlayerCameraPosition {
    /// Camera position relative to the player position.
    pub position: Vec3,
}

/// A player's camera rotation. Used by client to report its facing or for the server to dictate.
#[derive(NetworkMessage, ClientBound, ServerBound, Serialize, Deserialize, Debug, Clone)]
pub struct PlayerCameraRotation {
    /// Where the player camera is looking.
    pub rotation: Quat,
}

/// Send a left click to the server
#[derive(NetworkMessage, ServerBound, Serialize, Deserialize, Debug, Clone)]
pub struct LeftClick;

/// Send a right click to the server.
#[derive(NetworkMessage, ServerBound, Serialize, Deserialize, Debug, Clone)]
pub struct RightClick;

/// A chat message, sent by either the client or the server.
#[derive(NetworkMessage, ClientBound, ServerBound, Serialize, Deserialize, Clone, Debug)]
pub struct ChatMessage {
    /// Content of the message.
    pub message: String,
}
