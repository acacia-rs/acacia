//! Animated block textures: `textures/flipbook_textures.json` names the terrain textures whose image
//! is a vertical strip of frames, played in the texture's one array layer.

use std::collections::HashMap;
use std::path::Path;

use serde_json::Value;

use super::image::{TEXEL_BYTES, Texture};
use super::json;

/// How one terrain texture's strip plays.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Flipbook {
    /// Strip frames in play order; the strip's own order when absent.
    pub frames: Option<Vec<usize>>,
    pub ticks_per_frame: u32,
    /// Cross-fade between frames instead of stepping.
    pub blend: bool,
}

/// Flipbooks by terrain texture name. Only a texture's first variant is drawn, so only its flipbook is kept.
pub fn load(root: &Path) -> HashMap<String, Flipbook> {
    let entries = match json::read(&root.join("textures/flipbook_textures.json")) {
        Ok(Value::Array(entries)) => entries,
        Ok(_) => Vec::new(),
        Err(e) => {
            tracing::warn!(%e, "flipbooks");
            Vec::new()
        }
    };
    let number = |e: &Value, key: &str| e.get(key).and_then(Value::as_u64);
    let mut books = HashMap::new();
    for e in &entries {
        let Some(tile) = e.get("atlas_tile").and_then(Value::as_str) else { continue };
        if number(e, "atlas_index").unwrap_or(0) != 0 || number(e, "atlas_tile_variant").unwrap_or(0) != 0 {
            continue;
        }
        let frames = e.get("frames").and_then(Value::as_array).map(|f| f.iter().filter_map(Value::as_u64).map(|i| i as usize).collect());
        let book = Flipbook {
            frames,
            ticks_per_frame: number(e, "ticks_per_frame").unwrap_or(1).max(1) as u32,
            blend: e.get("blend_frames").and_then(Value::as_bool).unwrap_or(true),
        };
        books.entry(tile.to_owned()).or_insert(book);
    }
    books
}

/// One array layer's frames, in play order.
#[derive(Clone)]
pub struct Animation {
    pub layer: u16,
    frames: Vec<Texture>,
    ticks_per_frame: u32,
    blend: bool,
}

impl Animation {
    /// `None` when the strip holds a single frame.
    pub fn new(layer: u16, strip: Vec<Texture>, book: &Flipbook) -> Option<Animation> {
        let frames: Vec<Texture> = match &book.frames {
            Some(order) => order.iter().filter_map(|&i| strip.get(i).cloned()).collect(),
            None => strip,
        };
        (frames.len() > 1).then_some(Animation { layer, frames, ticks_per_frame: book.ticks_per_frame, blend: book.blend })
    }

    /// The layer's texels at a game tick (20 per second).
    pub fn at(&self, tick: u64) -> Texture {
        let per = u64::from(self.ticks_per_frame);
        let frame = |i: u64| &self.frames[(i % self.frames.len() as u64) as usize];
        let (step, into) = (tick / per, tick % per);
        let from = frame(step);
        if !self.blend || into == 0 {
            return from.clone();
        }
        let (to, t) = (frame(step + 1), into as f32 / per as f32);
        let mut rgba = Box::new([0; TEXEL_BYTES]);
        for (out, (a, b)) in rgba.iter_mut().zip(from.rgba.iter().zip(to.rgba.iter())) {
            *out = (f32::from(*a) + (f32::from(*b) - f32::from(*a)) * t).round() as u8;
        }
        Texture { rgba }
    }
}

/// The block texture array: one 16×16 layer per texture (layer 0 is the missing texture) and the
/// layers that animate.
#[derive(Clone, Default)]
pub struct Atlas {
    pub layers: Vec<Texture>,
    pub animations: Vec<Animation>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grey(v: u8) -> Texture {
        Texture { rgba: Box::new([v; TEXEL_BYTES]) }
    }

    #[test]
    fn frames_play_in_order_and_cross_fade() {
        let strip = vec![grey(0), grey(100), grey(200)];
        let book = Flipbook { frames: Some(vec![2, 0, 9]), ticks_per_frame: 4, blend: true };
        let a = Animation::new(7, strip.clone(), &book).unwrap();
        assert_eq!(a.at(0).rgba[0], 200);
        assert_eq!(a.at(1).rgba[0], 150, "a quarter of the way from 200 to 0");
        assert_eq!(a.at(4).rgba[0], 0);
        assert_eq!(a.at(8).rgba[0], 200, "the out-of-range frame is dropped, so it loops after two");

        let stepped = Animation::new(7, strip, &Flipbook { frames: None, ticks_per_frame: 4, blend: false }).unwrap();
        assert_eq!((stepped.at(3).rgba[0], stepped.at(4).rgba[0]), (0, 100));
        assert!(Animation::new(7, vec![grey(1)], &book).is_none());
    }
}
