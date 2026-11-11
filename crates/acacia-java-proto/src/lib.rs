//! Minecraft: Java Edition 26.3 packet codecs, network-free. Layouts are hand-written from the 26.3 codec classes;
//! packet ids are generated (`tools/gen_ids.mjs`).

pub mod actions;
pub mod chunk;
pub mod container;
pub mod dialog;
pub mod entity_types;
pub mod entity;
pub mod ids;
pub mod inventory;
pub mod item;
pub mod merchant;
pub mod nbt;
pub mod packets;
pub mod recipe_book;
pub mod recipes;
pub mod tag;
mod wire;

pub use wire::{Error, VarInt, Wire, read_varint, read_varlong, write_varint};

/// The protocol number of the pinned version.
pub const PROTOCOL_VERSION: i32 = 777;
