//! HUD, chat and menus for an Acacia client, laid out into [`DrawList`]s of textured quads that a
//! renderer draws over the world. No GPU and no window: themes differ in data, and layout is tested
//! headless. Design: README.md.

pub mod atlas;
pub mod chat;
pub mod debug;
pub mod draw;
pub mod font;
pub mod hud;
pub mod inventory;
pub mod lang;
pub mod menu;
pub mod overlay;
pub mod scale;
pub mod theme;

pub use atlas::{Atlas, Sprite};
pub use draw::{DrawList, Quad};
