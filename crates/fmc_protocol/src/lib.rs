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
    ParticleEmitter,
    DeleteParticleEmitter,
    Plugin,
    PluginData,
    // XXX: Always keep this at the bottom, occupies highest discriminant spot, so that when you
    // deserialize a MessageType, you can know that only values below 'MessageType::MAX as u8' are
    // valid.
    MAX,
}
