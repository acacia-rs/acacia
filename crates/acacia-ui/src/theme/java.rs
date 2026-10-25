//! The Java theme from the jar's unpacked `assets/minecraft` (tools/lookbake unpacks it), for what
//! the Java look still draws with the shared layouts: effect icons for screens, the book, fonts and
//! widgets. Its HUD draws through `crate::java` from the same files.

use std::path::Path;

use super::{Style, Theme, java_font, png};
use crate::atlas::Atlas;
use crate::effects;
use crate::widget::Widgets;
use crate::widget::java::Kit;

pub const STYLE: Style = Style { java_signs: true };

/// Loads what exists under `root`; missing sprites draw nothing.
pub fn load(root: &Path) -> Theme {
    let mut atlas = Atlas::new();
    for (_, name, _) in effects::EFFECTS {
        if let Some(image) = png(&root.join(format!("textures/mob_effect/{name}.png"))) {
            atlas.add(&effects::sprite(name), &image);
        }
    }
    // `book.png` holds the open book in its top-left 192 pixels.
    if let Some(sheet) = png(&root.join("textures/gui/book.png")) {
        atlas.add(crate::book::ART, &image::imageops::crop_imm(&sheet, 0, 0, 192, 192).to_image());
    }
    let font = java_font(root, &mut atlas);
    let galactic = super::java_galactic(root, &mut atlas);
    let widgets = Widgets::Java(Kit::load(root, &mut atlas));
    Theme { atlas, font, galactic, style: STYLE, widgets }
}
