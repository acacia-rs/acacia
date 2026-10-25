//! What the painter asks of the viewer for pictures: an entity is recorded for the renderer's
//! pictures texture (`app/portrait.rs` poses it), a banner is composed flat into the atlas.

use std::path::Path;

use acacia_render::banner::{self, Banner};
use acacia_ui::java::picture::Picture;
use acacia_ui::{Atlas, Quad, Sprite};

/// A picture-in-picture the renderer draws this frame: where (window pixels) and at what GUI scale.
#[derive(Debug, Clone)]
pub struct PictureRequest {
    pub picture: Picture,
    pub window: [f32; 4],
    pub gui_scale: f32,
}

/// The flag's front in the banner texture (64 × 64): `BannerFlagModel`'s 20 × 40 × 1 box's north
/// face. `GuiBannerResultRenderer` fills the picture with it: 16 pixels a block, the flag 44 pixels
/// above its pivot less the quarter block it is lifted, under `ITEMS_FLAT` lights that leave a
/// face towards the viewer unshaded.
const FLAG_FRONT: [f32; 4] = [1.0, 1.0, 21.0, 41.0];

/// The quad showing `picture` over `window`, or `None` when the viewer can't draw it.
pub fn quad(atlas: &mut Atlas, root: &Path, requests: &mut Vec<PictureRequest>, gui_scale: f32, picture: &Picture, window: [f32; 4]) -> Option<Quad> {
    match picture {
        Picture::Entity { .. } => {
            requests.push(PictureRequest { picture: picture.clone(), window, gui_scale });
            Some(Quad::picture(window))
        }
        Picture::Banner { base, patterns, .. } => {
            let sprite = banner_sprite(atlas, root, &Banner::from_java(base, patterns))?;
            let [l, t, r, b] = FLAG_FRONT.map(|v| v * sprite.width as f32 / 64.0);
            let (u, v) = (sprite.x as f32, sprite.y as f32);
            Some(Quad::plain(window, [u + l, v + t, u + r, v + b], acacia_ui::draw::WHITE))
        }
        // TODO: the enchanting table's book needs Java's BookModel from the entity models.
        _ => None,
    }
}

fn banner_sprite(atlas: &mut Atlas, root: &Path, banner: &Banner) -> Option<Sprite> {
    let name = format!("picture/banner/{banner:?}");
    if let Some(s) = atlas.get(&name) {
        return Some(s);
    }
    Some(atlas.add(&name, &banner::compose(root, banner)?))
}

/// A sprite of `atlas` whose ids are texture paths (`minecraft:entity/banner/creeper`).
pub fn atlas_sprite(images: &mut acacia_ui::java::paint::assets::Images, atlas: &mut Atlas, sprite: &str) -> Option<Sprite> {
    let path = sprite.split_once(':').map_or(sprite, |(_, p)| p);
    images.texture(atlas, path)
}
