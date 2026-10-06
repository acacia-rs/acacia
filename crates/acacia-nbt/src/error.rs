use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum Error {
    #[error("unexpected end of input (needed {needed} bytes, {remaining} remaining)")]
    Eof { needed: usize, remaining: usize },
    #[error("varint is too long")]
    VarIntTooLong,
    #[error("invalid length {0}")]
    InvalidLength(i64),
    #[error("unknown tag type {0}")]
    UnknownTag(u8),
    #[error("list of End tags with length {0}")]
    EndList(usize),
    #[error("nesting deeper than {0}")]
    TooDeep(usize),
}
