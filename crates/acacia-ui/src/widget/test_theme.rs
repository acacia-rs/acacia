//! Themes for headless tests: no sprites, and a font whose every glyph is 5 pixels wide (advance 6).

use std::path::Path;

use image::RgbaImage;

use super::Widgets;
use crate::atlas::Atlas;
use crate::font::Font;
use crate::theme::{Theme, bedrock, java};

pub fn theme(java_look: bool) -> Theme {
    let mut atlas = Atlas::new();
    let mut image = RgbaImage::new(128, 128);
    for cell in 0..256u32 {
        for x in 0..5 {
            image.put_pixel((cell % 16) * 8 + x, (cell / 16) * 8 + 3, image::Rgba([255; 4]));
        }
    }
    let page = atlas.add("font", &image);
    let rows = Font::code_page_rows();
    let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
    let font = Some(Font::from_grid(&image, page, &rows));
    let none = Path::new("");
    let (widgets, style) = if java_look {
        (Widgets::Java(super::java::Kit::load(none, &mut atlas)), java::STYLE)
    } else {
        (Widgets::Bedrock(super::bedrock::Kit::load(none, &mut atlas)), bedrock::STYLE)
    };
    Theme { atlas, font, style, widgets }
}
