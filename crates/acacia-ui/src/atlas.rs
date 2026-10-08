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
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
