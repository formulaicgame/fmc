#![deny(
    //missing_docs,
    missing_debug_implementations,
    // why does it need this
    //missing_copy_implementations,
    trivial_casts,
    trivial_numeric_casts,
    unsafe_code,
    unstable_features,
    unused_import_braces,
    unused_qualifications,
    clippy::unwrap_used
)]
#![allow(clippy::type_complexity)]

mod network_message;

pub mod messages;
pub use network_message::{ClientBound, ServerBound};

/// Storage type of blocks.
type BlockId = u16;

#[derive(Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum MessageType {
    ClientIdentification,
    ClientReady,
    AssetRequest,
    AssetResponse,
    Disconnect,
    ServerConfig,
    Time,
    Chunk,
    BlockUpdates,
    NewModel,
    DeleteModel,
    ModelPlayAnimation,
    ModelUpdateAsset,
    ModelUpdateTransform,
    ModelColor,
    SpawnCustomModel,
    LeftClick,
    RightClick,
    RenderDistance,
    PlayerAabb,
    PlayerCameraPosition,
    PlayerCameraRotation,
    PlayerPosition,
    InterfaceEquipItem,
    InterfaceInteraction,
    InterfaceItemBoxUpdate,
    InterfaceNodeVisibilityUpdate,
    InterfaceTextInput,
    InterfaceTextUpdate,
    InterfaceVisibilityUpdate,
    GuiSetting,
    EnableClientAudio,
    Sound,
    ParticleEffect,
    Plugin,
    PluginData,
    // XXX: Always keep this at the bottom, occupies highest discriminant spot, so that when you
    // deserialize a MessageType, you can know that only values below 'MessageType::MAX as u8' are
    // valid.
    MAX,
}
