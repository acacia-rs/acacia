//! Game sounds for an Acacia client: which samples a sound event plays (Bedrock's sound
//! definitions), decoding them (the pack's FSB5 files, Java's Ogg Vorbis) and positional playback.
//! Design: README.md.

pub mod defs;
pub mod fsb;
mod ogg;
pub mod player;
mod sounds;

pub use player::Listener;
pub use sounds::{Event, Sounds};
