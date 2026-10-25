//! The Java look's GUI, written as Java 26.3's own code draws it: layouts emit
//! `GuiGraphicsExtractor` calls ([`gui::GuiDraw`]) from the same state the game reads, the parity
//! probes compare those calls with the game's, and `paint` turns them into quads. Design:
//! README.md "Java GUI".

pub mod bidi;
pub mod chat;
pub mod container;
pub mod encode;
pub mod font;
pub mod gui;
pub mod hud;
pub mod lang;
pub mod mth;
pub mod paint;
pub mod picture;
pub mod screen;
pub mod split;
pub mod text;
pub mod tooltip;
pub mod widget;
