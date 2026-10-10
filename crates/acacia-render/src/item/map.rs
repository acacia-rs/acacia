//! A filled map held in first person: its picture on the pack's map sheet, held as Java's
//! `ItemInHandRenderer` holds it (`renderTwoHandedMap`, `renderOneHandedMap`, `renderMap`). The
//! arms under it, the swing and the markers on the picture are not drawn.

use std::path::Path;
use std::sync::Arc;

use glam::{Mat3, Mat4, Vec3};
use image::{Rgba, RgbaImage, imageops};

use crate::assets::image_file;
use crate::Camera;
use crate::entity::bake::{Joint, Mesh, Vertex};
use crate::entity::{EntityInstance, Layer, NO_MODEL, NO_TEXTURE, Pose, Skin};

/// Pixels along a map picture's side.
pub const PICTURE: u32 = 128;
/// The sheet reaches this far past the picture on each side.
const MARGIN: u32 = 7;
const SHEET: u32 = PICTURE + MARGIN * 2;
const BACKGROUND: &str = "textures/map/map_background";
const PARCHMENT: Rgba<u8> = Rgba([0xD6, 0xBE, 0x96, 0xFF]);
/// `renderMap`'s scale: the picture's side in view units.
const SIDE: f32 = 0.38;

/// A pointer on the picture. Neither pack here ships the game's marker images: each is drawn as
/// an arrowhead in its colour.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Marker {
    /// Picture pixels from its top left corner.
    pub at: [f32; 2],
    /// Degrees clockwise from pointing up the picture (north).
    pub turn: f32,
    pub colour: [u8; 3],
}

/// Half the arrowhead's length in picture pixels, and how far its dark rim reaches past it.
const ARROW: f32 = 3.5;
const RIM: f32 = 1.0;

fn mark(picture: &mut RgbaImage, marker: &Marker) {
    let (sin, cos) = marker.turn.to_radians().sin_cos();
    // Signed distance outside an arrowhead pointing up: its tip at -ARROW, its base at +ARROW.
    let outside = |x: f32, y: f32| {
        let slope = (x.abs() * 2.0 * ARROW - (y + ARROW) * ARROW * 0.75) / (ARROW * 2.14);
        slope.max(y - ARROW).max(-ARROW - y)
    };
    let reach = (ARROW + RIM).ceil() as i32;
    for (dx, dy) in (-reach..=reach).flat_map(|dy| (-reach..=reach).map(move |dx| (dx, dy))) {
        let (px, py) = (marker.at[0].round() as i32 + dx, marker.at[1].round() as i32 + dy);
        if px < 0 || py < 0 || px >= PICTURE as i32 || py >= PICTURE as i32 {
            continue;
        }
        // The pixel in the arrow's own frame.
        let (x, y) = (dx as f32 * cos + dy as f32 * sin, dy as f32 * cos - dx as f32 * sin);
        let distance = outside(x, y);
        if distance <= RIM {
            let [r, g, b] = if distance <= 0.0 { marker.colour } else { [0x20; 3] };
            picture.put_pixel(px as u32, py as u32, Rgba([r, g, b, 255]));
        }
    }
}

fn marked(picture: &[u8], markers: &[Marker]) -> Option<RgbaImage> {
    let mut picture = RgbaImage::from_raw(PICTURE, PICTURE, picture.to_vec())?;
    markers.iter().for_each(|marker| mark(&mut picture, marker));
    Some(picture)
}

/// The sheet with `picture` (RGBA8, [`PICTURE`] squared) and its markers on it; `None` for
/// another size.
pub fn skin(root: &Path, picture: &[u8], markers: &[Marker]) -> Option<Skin> {
    let picture = marked(picture, markers)?;
    let background = image_file(root, BACKGROUND).and_then(|file| image::open(file).ok()).map(|image| image.into_rgba8());
    let mut sheet = match background {
        Some(image) => imageops::resize(&image, SHEET, SHEET, imageops::FilterType::Nearest),
        None => RgbaImage::from_pixel(SHEET, SHEET, PARCHMENT),
    };
    imageops::overlay(&mut sheet, &picture, MARGIN.into(), MARGIN.into());
    Some(Skin { width: SHEET, height: SHEET, rgba: sheet.into_raw(), mesh: Some(sheet_mesh()) })
}

/// `picture` alone on the unit square, as an item frame shows it; unexplored pixels as parchment.
pub fn picture(picture: &[u8], markers: &[Marker]) -> Option<Skin> {
    let mut sheet = RgbaImage::from_pixel(PICTURE, PICTURE, PARCHMENT);
    imageops::overlay(&mut sheet, &marked(picture, markers)?, 0, 0);
    Some(Skin { width: PICTURE, height: PICTURE, rgba: sheet.into_raw(), mesh: Some(sheet_mesh()) })
}

/// A unit square facing +z, the image's top row along its top edge, drawn from both sides.
fn sheet_mesh() -> Mesh {
    let corner = |x: f32, y: f32, facing: f32| Vertex { position: [x - 0.5, 0.5 - y, 0.0], bone: 0, normal: [0.0, 0.0, facing], uv: [x, y] };
    let side = |facing: f32| {
        let c = [corner(0.0, 0.0, facing), corner(0.0, 1.0, facing), corner(1.0, 1.0, facing), corner(1.0, 0.0, facing)];
        if facing > 0.0 { [c[0], c[1], c[2], c[0], c[2], c[3]] } else { [c[0], c[2], c[1], c[0], c[3], c[2]] }
    };
    let joint = Joint { parent: None, pivot: Vec3::ZERO, rotation: [0.0; 3], unbind: Mat4::IDENTITY };
    Mesh { vertices: [side(1.0), side(-1.0)].concat(), bones: vec!["root".into()], joints: vec![joint] }
}

/// How the map is held.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hold {
    /// In the main hand with the other one empty: before the face, lying flat until the holder
    /// looks down.
    Both,
    /// Beside the view, in the hand that has it.
    One { left: bool },
}

/// The sheet's unit square to view space (x right, y up, looking down -z). `pitch` in degrees,
/// positive looking down.
fn frame(hold: Hold, pitch: f32) -> Mat4 {
    let sheet = Mat4::from_scale(Vec3::splat(SIDE * SHEET as f32 / PICTURE as f32));
    match hold {
        Hold::Both => {
            // `calculateMapTilt`: 1 lying flat (looking ahead) to 0 upright (49.5° down or more).
            let tilt = 0.5 - ((1.0 - pitch / 45.0 + 0.1).clamp(0.0, 1.0) * std::f32::consts::PI).cos() * 0.5;
            Mat4::from_translation(Vec3::new(0.0, 0.04 - tilt * 0.5, -0.72)) * Mat4::from_rotation_x((tilt * -85.0).to_radians()) * Mat4::from_scale(Vec3::splat(2.0)) * sheet
        }
        Hold::One { left } => Mat4::from_translation(Vec3::new(if left { -0.635 } else { 0.635 }, -0.205, -0.75)) * sheet,
    }
}

pub fn first_person(skin: &Arc<Skin>, camera: &Camera, hold: Hold) -> EntityInstance {
    let (forward, right) = (camera.forward(), camera.right());
    let view = Mat4::from_mat3(Mat3::from_cols(right, right.cross(forward), -forward));
    let pitch = -forward.y.clamp(-1.0, 1.0).asin().to_degrees();
    EntityInstance {
        layers: [Layer::plain(NO_MODEL, NO_TEXTURE)].into(),
        skin: Some(skin.clone()),
        position: camera.position,
        yaw: 0.0,
        scale: 1.0,
        pose: Pose::default(),
        frame: Some(view * frame(hold, pitch)),
        hurt: false,
        glint: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_picture_sits_inside_the_sheet_s_margin() {
        let mut picture = vec![0u8; (PICTURE * PICTURE * 4) as usize];
        picture[..4].copy_from_slice(&[1, 2, 3, 255]);
        // A marker in the far corner, pointing east: its tip is a white pixel right of its middle.
        let east = Marker { at: [100.0, 100.0], turn: 90.0, colour: [255; 3] };
        let skin = skin(Path::new("no-such-pack"), &picture, &[east]).unwrap();
        let texel = |x: u32, y: u32| &skin.rgba[((y * SHEET + x) * 4) as usize..][..4];
        // An unexplored (transparent) pixel leaves the parchment.
        assert_eq!((texel(MARGIN, MARGIN), texel(MARGIN + 1, MARGIN), texel(0, 0)), (&[1, 2, 3, 255][..], &PARCHMENT.0[..], &PARCHMENT.0[..]));
        assert_eq!((texel(MARGIN + 103, MARGIN + 100), texel(MARGIN + 105, MARGIN + 100)), (&[255, 255, 255, 255][..], &PARCHMENT.0[..]));
        assert_eq!(texel(MARGIN + 100, MARGIN + 102), &[0x20, 0x20, 0x20, 255][..], "the rim past its flank");
        assert!(super::skin(Path::new("no-such-pack"), &picture[4..], &[]).is_none());
    }

    #[test]
    fn a_map_in_both_hands_rises_as_the_holder_looks_down() {
        let top = |pitch| frame(Hold::Both, pitch).transform_point3(Vec3::new(0.0, 0.5, 0.0));
        let centre = |hold, pitch| frame(hold, pitch).transform_point3(Vec3::ZERO);
        // Looking ahead it lies nearly flat, low in the view: its far edge is deeper, not higher.
        let (flat, upright) = (top(0.0), top(60.0));
        assert!(flat.z < -1.0 && flat.y < -0.4, "{flat}");
        assert!((upright.z + 0.72).abs() < 1e-4 && upright.y > 0.4, "{upright}");
        // Upright from 49.5° down (the 0.1 in `calculateMapTilt`).
        assert_eq!(centre(Hold::Both, 50.0), centre(Hold::Both, 80.0));
        // One hand holds it half the size, to its side.
        let (right, left) = (centre(Hold::One { left: false }, 0.0), centre(Hold::One { left: true }, 0.0));
        assert!(right.x > 0.5 && (right.x + left.x).abs() < 1e-6, "{right} {left}");
    }
}
