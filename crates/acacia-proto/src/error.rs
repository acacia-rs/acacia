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
    #[error("{at}: {source}")]
    Context {
        at: &'static str,
        source: Box<DecodeError>,
    },
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
