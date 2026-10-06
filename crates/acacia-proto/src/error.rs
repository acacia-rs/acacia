use thiserror::Error;

/// Decoding failure. Context (`at`) is only attached on the error path, so the happy path never allocates.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum DecodeError {
    #[error("unexpected end of input (needed {needed} bytes, {remaining} remaining)")]
    Eof { needed: usize, remaining: usize },
    #[error("varint is too long")]
    VarIntTooLong,
    #[error("invalid length {0}")]
    InvalidLength(i64),
    #[error("invalid nbt: {0}")]
    Nbt(&'static str),
    #[error("nbt nesting deeper than {0}")]
    NbtTooDeep(usize),
    #[error("packet id mismatch: expected {expected}, got {actual}")]
    IdMismatch { expected: u32, actual: u32 },
    #[error("{0} bytes after the packet body")]
    TrailingBytes(usize),
    #[error("{0}")]
    Lenient(crate::strict::Leniency),
    #[error("no packet has id {0}")]
    UnknownPacket(u32),
    #[error("{at}: {source}")]
    Context {
        at: &'static str,
        source: Box<DecodeError>,
    },
}

impl From<acacia_nbt::Error> for DecodeError {
    fn from(e: acacia_nbt::Error) -> Self {
        use acacia_nbt::Error;
        match e {
            Error::Eof { needed, remaining } => DecodeError::Eof { needed, remaining },
            Error::VarIntTooLong => DecodeError::VarIntTooLong,
            Error::InvalidLength(n) => DecodeError::InvalidLength(n),
            Error::UnknownTag(_) => DecodeError::Nbt("unknown tag type"),
            Error::EndList(_) => DecodeError::Nbt("list of End tags has a length"),
            Error::TooDeep(max) => DecodeError::NbtTooDeep(max),
        }
    }
}

impl DecodeError {
    /// Wraps the error with a `Type.field` location.
    #[cold]
    pub fn at(self, at: &'static str) -> Self {
        DecodeError::Context {
            at,
            source: Box::new(self),
        }
    }

    /// The innermost error, with all context stripped.
    pub fn root(&self) -> &DecodeError {
        match self {
            DecodeError::Context { source, .. } => source.root(),
            e => e,
        }
    }
}
