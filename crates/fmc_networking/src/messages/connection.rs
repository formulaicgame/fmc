use bevy::prelude::Resource;
use fmc_networking_derive::{ClientBound, NetworkMessage, ServerBound};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::BlockId;

/// Sent by client to notify the server that it has processed all assets and is ready to be served.
#[derive(NetworkMessage, ServerBound, Serialize, Deserialize, Debug, Clone)]
pub struct ClientFinishedLoading;

/// Initial server configuration needed for client setup.
#[derive(Resource, NetworkMessage, ClientBound, Serialize, Deserialize, Debug, Clone)]
pub struct ServerConfig {
    /// Hash of the assets the server wants used.
    pub assets_hash: Vec<u8>,
    /// Map from block name to id on the server.
    pub block_ids: HashMap<String, BlockId>,
    /// Map from model name to id on the server.
    pub model_ids: HashMap<String, u32>,
    /// Map from item name to id on the server.
    pub item_ids: HashMap<String, u32>,
    /// Maximum render distance allowed by server, measured in chunks.
    pub render_distance: u32,
}

/// Clients send this immediately on established connection to identify themselves.
#[derive(NetworkMessage, ServerBound, Serialize, Deserialize, Debug)]
pub struct ClientIdentification {
    /// The name the player wants to use.
    pub name: String,
}

/// Forceful disconnection by the server.
#[derive(NetworkMessage, ClientBound, Serialize, Deserialize, Debug)]
pub struct Disconnect {
    /// Reason for the disconnect, optional
    pub message: String,
}

// TODO: This is meant to be temporary. As day/night is defined client-side, the server only sends
// the time of day (as angle of sun).
/// Sets the time of day.
#[derive(NetworkMessage, ClientBound, Serialize, Deserialize, Debug, Clone)]
pub struct Time {
    /// Angle of the sun
    pub angle: f32,
}

/// A set of assets from the server
#[derive(NetworkMessage, ClientBound, Serialize, Deserialize, Debug)]
pub struct AssetResponse {
    /// Assets stored as a tarball
    pub file: Vec<u8>,
}

/// Sent by clients if they don't have assets (or the wrong ones).
#[derive(NetworkMessage, ServerBound, Serialize, Deserialize, Debug, Clone, Copy)]
pub struct AssetRequest;
