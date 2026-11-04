/// The chunk size expected to be transmitted over the protocol.
pub const PROTO_CHUNK_SIZE: u8 = 16;

pub(crate) const LOCAL_SHIFT: u32 = (PROTO_CHUNK_SIZE - 1).bit_width();
pub(crate) const LOCAL_MASK: u8 = (1 << (LOCAL_SHIFT + 1)) - 1;
