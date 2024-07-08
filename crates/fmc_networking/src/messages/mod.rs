/// Everything that happens on connection and disconnection
mod connection;
pub use connection::{
    AssetRequest, AssetResponse, ClientFinishedLoading, ClientIdentification, Disconnect,
    ServerConfig, Time,
};

/// Chunk management
mod chunk;
pub use chunk::Chunk;

/// Individual changes to blocks
mod blocks;
pub use blocks::BlockUpdates;

/// Things like players, the sun/skybox, arrows. Everything that is not a block.
mod models;
pub use models::{
    DeleteModel, ModelPlayAnimation, ModelUpdateAsset, ModelUpdateTransform, NewModel,
};

/// Changes to the player.
mod player;
pub use player::{
    LeftClick, PlayerAabb, PlayerCameraPosition, PlayerCameraRotation, PlayerPosition, RightClick,
};

/// User interface
mod interfaces;
pub use interfaces::{
    InterfaceClose, InterfaceEquipItem, InterfaceInteraction, InterfaceItemBoxUpdate,
    InterfaceOpen, InterfaceTextBoxUpdate, InterfaceTextInput, InterfaceVisibilityUpdate,
};

mod audio;
pub use audio::{EnableClientAudio, Sound};
