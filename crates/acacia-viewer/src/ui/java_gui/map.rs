//! `GuiGraphicsExtractor.map`: a map's picture with its markers, for the cartography table's
//! preview. The painter's quads stay axis-aligned, so the markers (turned in sixteenths of a turn)
//! are drawn into the picture here, at [`SUB`] texels a map pixel, as `map`'s posed blits place
//! them; the picture goes into the atlas once per change of the bot's map.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use acacia_bot::proto::types::MapDecorationType as Kind;
use acacia_bot::state::{MAP_SIZE, MapImage, MapMarker};
use acacia_ui::java::gui::{Blend, GuiDraw};
use acacia_ui::{Atlas, Sprite};
use image::{Rgba, RgbaImage};

/// The texture path [`draws`] blits, answered by [`MapPicture::sprite`].
pub const TEXTURE: &str = "acacia/map_picture";
/// Texels per map pixel in the composed picture.
const SUB: u32 = 4;

/// The Java `map_decorations` sprite a Bedrock marker shows, `None` for those Java has no sprite
/// for (signs, the green triangle) or doesn't draw. Bedrock colours other players' markers; Java
/// draws every player white.
pub fn sprite(kind: Kind) -> Option<&'static str> {
    Some(match kind {
        Kind::MarkerWhite | Kind::MarkerWhite2 | Kind::MarkerPink | Kind::MarkerPink2 | Kind::MarkerOrange | Kind::MarkerOrange2 => "player",
        Kind::MarkerYellow | Kind::MarkerYellow2 | Kind::MarkerTeal | Kind::MarkerTeal2 => "player",
        Kind::MarkerGreen | Kind::MarkerGreen2 => "frame",
        Kind::MarkerRed | Kind::MarkerRed2 => "red_marker",
        Kind::MarkerBlue | Kind::MarkerBlue2 => "blue_marker",
        Kind::CrossWhite | Kind::CrossWhite2 => "target_x",
        Kind::TriangleRed | Kind::TriangleRed2 => "target_point",
        Kind::SquareWhite | Kind::SquareWhite2 => "player_off_map",
        Kind::SmallSquareWhite | Kind::SmallSquareWhite2 => "player_off_limits",
        Kind::Mansion | Kind::Mansion2 => "woodland_mansion",
        Kind::Monument | Kind::Monument2 => "ocean_monument",
        Kind::VillageDesert | Kind::VillageDesert2 => "desert_village",
        Kind::VillagePlains | Kind::VillagePlains2 => "plains_village",
        Kind::VillageSavanna | Kind::VillageSavanna2 => "savanna_village",
        Kind::VillageSnowy | Kind::VillageSnowy2 => "snowy_village",
        Kind::VillageTaiga | Kind::VillageTaiga2 => "taiga_village",
        Kind::JungleTemple | Kind::JungleTemple2 => "jungle_temple",
        Kind::WitchHut | Kind::WitchHut2 => "swamp_hut",
        _ => return None,
    })
}

/// `map`'s draws at the pose's origin: the picture, 128 pixels square.
pub fn draws() -> Vec<GuiDraw> {
    let side = MAP_SIZE as i32;
    vec![GuiDraw::Blit { texture: TEXTURE.into(), blend: Blend::Textured, x: 0, y: 0, w: side, h: side, uv: [0.0, 0.0], src: [side, side], tex: [side, side], color: 0xFFFF_FFFF }]
}

/// The map last put in the atlas, and the marker images read so far.
#[derive(Default)]
pub struct MapPicture {
    shown: Option<Arc<MapImage>>,
    markers: HashMap<&'static str, Option<RgbaImage>>,
}

impl MapPicture {
    /// Puts `map` in the atlas unless it is already there.
    pub fn show(&mut self, atlas: &mut Atlas, root: &Path, map: &Arc<MapImage>) {
        if self.shown.as_ref().is_some_and(|s| Arc::ptr_eq(s, map)) {
            return;
        }
        let picture = compose(map, |name| {
            self.markers.entry(name).or_insert_with(|| image::open(root.join(format!("textures/map/decorations/{name}.png"))).ok().map(|i| i.into_rgba8())).clone()
        });
        atlas.replace(TEXTURE, &picture);
        self.shown = Some(map.clone());
    }

    pub fn sprite(&self, atlas: &Atlas) -> Option<Sprite> {
        self.shown.as_ref().and(atlas.get(TEXTURE))
    }
}

/// The picture scaled up [`SUB`] times with each marker blitted over it.
fn compose(map: &MapImage, mut marker_image: impl FnMut(&'static str) -> Option<RgbaImage>) -> RgbaImage {
    let side = MAP_SIZE as u32;
    let mut out = RgbaImage::from_fn(side * SUB, side * SUB, |x, y| {
        let at = ((y / SUB * side + x / SUB) * 4) as usize;
        Rgba(map.rgba[at..at + 4].try_into().unwrap_or([0; 4]))
    });
    for m in &map.markers {
        if let Some(image) = sprite(m.kind).and_then(&mut marker_image) {
            stamp(&mut out, &image, m);
        }
    }
    out
}

/// One marker as `map` blits it: posed by `translate(x/2+64, y/2+64)`, `rotate(rot*22.5°)`,
/// `scale(4)`, `translate(-0.125, 0.125)`, over -1..1 with its V flipped; each texel sampled at
/// its middle.
fn stamp(out: &mut RgbaImage, image: &RgbaImage, m: &MapMarker) {
    let middle = MAP_SIZE as f32 / 2.0;
    let centre = m.offset.map(|half_pixels| f32::from(half_pixels) / 2.0 + middle);
    let (sin, cos) = (f32::from(m.rotation) * 22.5f32).to_radians().sin_cos();
    // The quad reaches 4.5 · √2 map pixels from its centre at most.
    let (reach, width) = (7.0, out.width());
    let span = |c: f32| ((c - reach).max(0.0) * SUB as f32) as u32..(((c + reach) * SUB as f32) as u32).min(width);
    for py in span(centre[1]) {
        for px in span(centre[0]) {
            let (dx, dy) = ((px as f32 + 0.5) / SUB as f32 - centre[0], (py as f32 + 0.5) / SUB as f32 - centre[1]);
            let (qx, qy) = (cos * dx + sin * dy, cos * dy - sin * dx);
            let (vx, vy) = (qx / 4.0 + 0.125, qy / 4.0 - 0.125);
            let (u, v) = ((vx + 1.0) / 2.0, (1.0 - vy) / 2.0);
            if !(0.0..1.0).contains(&u) || !(0.0..1.0).contains(&v) {
                continue;
            }
            let texel = *image.get_pixel((u * image.width() as f32) as u32, (v * image.height() as f32) as u32);
            over(out.get_pixel_mut(px, py), texel);
        }
    }
}

/// `GUI_TEXTURED`'s straight alpha blend.
fn over(dst: &mut Rgba<u8>, src: Rgba<u8>) {
    let a = f32::from(src[3]) / 255.0;
    let out_a = a + f32::from(dst[3]) / 255.0 * (1.0 - a);
    for i in 0..3 {
        dst[i] = (f32::from(src[i]) * a + f32::from(dst[i]) * (1.0 - a)).round() as u8;
    }
    dst[3] = (out_a * 255.0).round() as u8;
}

#[cfg(test)]
#[path = "map_tests.rs"]
mod tests;
