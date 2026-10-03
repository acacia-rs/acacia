#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("empty batch")]
    EmptyBatch,
    #[error("message 0x{0:02x} is not a game batch")]
    NotGameMessage(u8),
    #[error("malformed batch framing")]
    MalformedBatch,
    #[error("batch checksum mismatch")]
    BadChecksum,
    #[error("unknown compression id 0x{0:02x}")]
    UnknownCompression(u8),
    #[error("decompressed batch exceeds limit")]
    BatchTooLarge,
    #[error("decompression failed: {0}")]
    Decompress(String),
    #[error(transparent)]
    Decode(#[from] acacia_proto::DecodeError),
    #[error("server handshake: {0}")]
    Handshake(String),
    #[error("client login: {0}")]
    Login(String),
}
