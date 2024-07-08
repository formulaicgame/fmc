use fmc_networking_derive::{ClientBound, NetworkMessage, ServerBound};
use serde::{Deserialize, Serialize};

/// A set of assets from the server
#[derive(NetworkMessage, ClientBound, Serialize, Deserialize, Debug)]
pub struct AssetResponse {
    /// Assets stored as a tarball
    pub file: Vec<u8>,
}

/// Sent by clients if they don't have assets (or the wrong ones).
#[derive(NetworkMessage, ServerBound, Serialize, Deserialize, Debug, Clone, Copy)]
pub struct AssetRequest;
