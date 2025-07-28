use crate::MessageType;

/// A marker trait to signal that this message should be sent to the server
pub trait ServerBound {
    const TYPE: MessageType;
}

/// A marker trait to signal that this message should be sent to clients
pub trait ClientBound {
    const TYPE: MessageType;
}
