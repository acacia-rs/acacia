//! Writes a shield's slot icon bare and with two banners, side by side and enlarged, to look at
//! what a pack's `shield_patterns` compose to: no command makes a patterned shield.
//! `cargo run -p acacia-render --example shield_icon -- <pack root> <out.png>`
use std::path::PathBuf;

use acacia_render::banner::Banner;
use acacia_render::item::model_icon;
use image::{RgbaImage, imageops};

const SCALE: u32 = 8;

fn main() {
    let mut args = std::env::args().skip(1);
    let root = PathBuf::from(args.next().unwrap_or_else(|| "assets/vanilla".into()));
    let out = args.next().unwrap_or_else(|| "shield_icon.png".into());
    // Bedrock's dye numbers: red cloth with a black border and a white cross; a blue one with a creeper.
    let banners = [None, Some(Banner::from_bedrock(1, [("bo", 0), ("cr", 15)], 0)), Some(Banner::from_bedrock(4, [("cre", 10)], 0))];
    let icons: Vec<RgbaImage> = banners.iter().filter_map(|banner| model_icon(&root, "minecraft:shield", banner.as_ref())).collect();
    let Some(first) = icons.first() else { return eprintln!("no shield texture under {}", root.display()) };
    let (width, height) = first.dimensions();
    let mut sheet = RgbaImage::from_pixel(width * icons.len() as u32, height, image::Rgba([0x8B, 0x8B, 0x8B, 0xFF]));
    for (index, icon) in icons.iter().enumerate() {
        imageops::overlay(&mut sheet, icon, i64::from(width) * index as i64, 0);
    }
    let sheet = imageops::resize(&sheet, sheet.width() * SCALE, height * SCALE, imageops::FilterType::Nearest);
    sheet.save(&out).unwrap_or_else(|e| eprintln!("{out}: {e}"));
    println!("{} icons to {out}", icons.len());
}
