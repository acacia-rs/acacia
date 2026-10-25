//! Every UI image in one texture: sprites and font pages packed onto shelves, by name.

use std::collections::HashMap;

use image::RgbaImage;

/// Atlas width; it grows downwards.
const WIDTH: u32 = 1024;
/// Texels left between images, so filtering at a scale never reaches a neighbour.
const GAP: u32 = 1;
pub const WHITE: &str = "white";

/// Where an image lies in the atlas, in texels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Sprite {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

pub struct Atlas {
    image: RgbaImage,
    sprites: HashMap<String, Sprite>,
    /// Top and height of the open shelf, and how far along it is filled.
    shelf: (u32, u32, u32),
    /// Bumped on every change, so the renderer knows to upload again.
    pub version: u64,
}

impl Default for Atlas {
    fn default() -> Self {
        Atlas::new()
    }
}

impl Atlas {
    /// An atlas holding only [`WHITE`], an opaque white texel for flat fills.
    pub fn new() -> Atlas {
        let mut atlas = Atlas { image: RgbaImage::new(WIDTH, 1), sprites: HashMap::new(), shelf: (0, 0, 0), version: 0 };
        atlas.add(WHITE, &RgbaImage::from_pixel(1, 1, image::Rgba([255; 4])));
        atlas
    }

    pub fn image(&self) -> &RgbaImage {
        &self.image
    }

    pub fn get(&self, name: &str) -> Option<Sprite> {
        self.sprites.get(name).copied()
    }

    /// Whether `rect` (texels, either corner order) lies inside one sprite.
    pub fn holds(&self, [l, t, r, b]: [f32; 4]) -> bool {
        let (x0, x1, y0, y1) = (l.min(r), l.max(r), t.min(b), t.max(b));
        let e = 1e-3;
        self.sprites.values().any(|s| {
            let (sx, sy) = (s.x as f32, s.y as f32);
            x0 >= sx - e && y0 >= sy - e && x1 <= sx + s.width as f32 + e && y1 <= sy + s.height as f32 + e
        })
    }

    /// The quads whose atlas lookups leave their sprite: a repeating texture without `wrap`, a
    /// UV past its image. Pictures sample their own texture.
    pub fn stray<'q>(&self, quads: &'q [crate::Quad]) -> impl Iterator<Item = &'q crate::Quad> {
        quads.iter().filter(|q| !q.picture && !self.holds(q.wrap.unwrap_or(q.uv)))
    }

    /// [`WHITE`]'s sprite.
    pub fn white(&self) -> Sprite {
        self.sprites[WHITE]
    }

    /// Packs `image` under `name`, replacing nothing: a name added twice keeps its first image.
    pub fn add(&mut self, name: &str, image: &RgbaImage) -> Sprite {
        if let Some(&sprite) = self.sprites.get(name) {
            return sprite;
        }
        let (w, h) = (image.width().min(WIDTH), image.height());
        let (mut top, mut height, mut used) = self.shelf;
        if used + w > WIDTH {
            (top, height, used) = (top + height + GAP, 0, 0);
        }
        height = height.max(h);
        let sprite = Sprite { x: used, y: top, width: w, height: h };
        if top + height > self.image.height() {
            let mut grown = RgbaImage::new(WIDTH, (top + height).next_power_of_two());
            image::imageops::replace(&mut grown, &self.image, 0, 0);
            self.image = grown;
        }
        image::imageops::replace(&mut self.image, image, i64::from(sprite.x), i64::from(sprite.y));
        self.shelf = (top, height, used + w + GAP);
        self.sprites.insert(name.to_owned(), sprite);
        self.version += 1;
        sprite
    }

    /// Packs `image` under `name`, painting over the image there when it is the same size (a
    /// picture that changes, such as a map's).
    pub fn replace(&mut self, name: &str, image: &RgbaImage) -> Sprite {
        match self.sprites.get(name) {
            Some(&s) if (s.width, s.height) == image.dimensions() => {
                image::imageops::replace(&mut self.image, image, i64::from(s.x), i64::from(s.y));
                self.version += 1;
                s
            }
            _ => {
                self.sprites.remove(name);
                self.add(name, image)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_repeating_quad_is_stray_until_it_wraps_within_its_sprite() {
        let mut atlas = Atlas::new();
        let s = atlas.add("bg", &RgbaImage::new(16, 16));
        let (x, y) = (s.x as f32, s.y as f32);
        let sprite = [x, y, x + 16.0, y + 16.0];
        let tiled = crate::Quad::plain([0.0; 4], [x, y, x + 64.0, y + 16.0], [255; 4]);
        assert_eq!(atlas.stray(&[tiled]).count(), 1);
        assert_eq!(atlas.stray(&[crate::Quad { wrap: Some(sprite), ..tiled }]).count(), 0);
    }

    #[test]
    fn images_never_overlap() {
        let mut atlas = Atlas::new();
        let sizes = [(182, 22), (24, 23), (9, 9), (1000, 5), (128, 128), (9, 9)];
        let sprites: Vec<Sprite> = sizes.iter().enumerate().map(|(i, &(w, h))| atlas.add(&format!("s{i}"), &RgbaImage::new(w, h))).collect();
        for (i, a) in sprites.iter().enumerate() {
            assert!(a.x + a.width <= WIDTH && a.y + a.height <= atlas.image().height());
            for b in &sprites[i + 1..] {
                let apart = a.x + a.width <= b.x || b.x + b.width <= a.x || a.y + a.height <= b.y || b.y + b.height <= a.y;
                assert!(apart, "{a:?} {b:?}");
            }
        }
        assert_eq!(atlas.get("s2"), Some(sprites[2]));
        assert_eq!(atlas.image().get_pixel(atlas.white().x, atlas.white().y).0, [255; 4]);
    }

    #[test]
    fn a_replaced_image_keeps_its_place() {
        let mut atlas = Atlas::new();
        let first = atlas.replace("map", &RgbaImage::from_pixel(4, 4, image::Rgba([1; 4])));
        let version = atlas.version;
        assert_eq!(atlas.replace("map", &RgbaImage::from_pixel(4, 4, image::Rgba([2; 4]))), first);
        assert_eq!(atlas.image().get_pixel(first.x + 3, first.y + 3).0, [2; 4]);
        assert!(atlas.version > version);
        assert_eq!(atlas.replace("map", &RgbaImage::new(8, 8)).width, 8);
    }
}
