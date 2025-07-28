use bevy::prelude::Event;
use serde::{Deserialize, Serialize};

use fmc_protocol_derive::ClientBound;

/// Enable or disable a client plugin
#[derive(ClientBound, Event, Serialize, Deserialize, Debug, Clone)]
pub enum Plugin {
    Enable(String),
    Disable(String),
}

/// Send data to a plugin
#[derive(ClientBound, Event, Serialize, Deserialize, Debug, Clone)]
pub struct PluginData {
    /// The name of the plugin
    pub plugin: String,
    /// The data the plugin will receive
    pub data: Vec<u8>,
}
