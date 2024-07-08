use downcast_rs::DowncastSync;

#[typetag::serde]
/// Any type that should be sent over the wire has to implement [`NetworkMessage`].
///
/// ## Example
/// ```rust
/// use fmc_networking::NetworkMessage;
/// use serde::{Serialize, Deserialize};
///
/// #[derive(Serialize, Deserialize)]
/// struct PlayerInformation {
///     health: usize,
///     position: (u32, u32, u32)
/// }
///
/// #[typetag::serde]
/// impl NetworkMessage for PlayerInformation {}
/// ```
/// You will also have to mark it with either [`ServerBound`] or [`ClientBound`] (or both)
/// to signal which direction this message can be sent.
pub trait NetworkMessage: DowncastSync {}

downcast_rs::impl_downcast!(sync NetworkMessage);

/**
A marker trait to signal that this message should be sent to the server

## Note

You can implement both [`ServerMessage`] and [`ClientMessage`]
*/
pub trait ServerBound: NetworkMessage {
    /// A unique name to identify your message, this needs to be unique __across all included crates__
    ///
    /// A good combination is crate name + struct name
    const NAME: &'static str;
}

/**
A marker trait to signal that this message should be sent to clients

## Note

You can implement both [`ClientMessage`] and [`ServerMessage`]
*/
pub trait ClientBound: NetworkMessage {
    /// A unique name to identify your message, this needs to be unique __across all included crates__
    ///
    /// A good combination is crate name + struct name
    const NAME: &'static str;
}
