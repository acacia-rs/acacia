mod congestion;
mod recv;
mod send;

pub use recv::{RecvError, RecvLimits};
pub(crate) use recv::RecvState;
pub(crate) use send::SendQueue;

pub const CHANNELS: usize = 32;

#[cfg(test)]
mod tests;
