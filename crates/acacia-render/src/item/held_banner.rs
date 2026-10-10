//! A banner as an item: the standing banner's model in the stack's cloth, placed by Java's
//! `item/template_banner.json` displays (26.3). Java stands the model on the bottom of the
//! item's box, in its middle.

use std::path::Path;

use glam::{Mat4, Vec3};

use super::hand::Display;
use crate::banner::{self, Banner};
use crate::entity::Skin;
use crate::entity::block_models::standing_banner;

/// `ground`: scale, then lift in blocks.
pub(super) const GROUND: (f32, f32) = (0.25, 1.0 / 16.0);

pub(super) fn skin(root: &Path, cloth: &Banner) -> Option<Skin> {
    let image = banner::compose(root, cloth)?;
    let mesh = standing_banner()?.fixed(in_item_space());
    Some(Skin { width: image.width(), height: image.height(), rgba: image.into_raw(), mesh: Some(mesh) })
}

/// The block's mesh to item mesh space: onto the box's floor, the cloth's front where a slot
/// shows it (`gui` turns it 20°). The hands' 90° then holds it side-on: seen from behind in
/// first person, and pointing forward out of another's fist.
fn in_item_space() -> Mat4 {
    Mat4::from_translation(Vec3::new(0.0, -0.5, 0.0))
}

/// The same entry for either hand: the file lists the right one only.
pub(super) fn display(first_person: bool) -> Display {
    if first_person { Display::px([0.0, 90.0, 0.0], [0.0; 3], 0.375) } else { Display::px([0.0, 90.0, 0.0], [0.0, 2.0, 0.5], 0.375) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_model_stands_on_the_bottom_of_the_item_s_box() {
        let mesh = standing_banner().unwrap().fixed(in_item_space());
        let ys = mesh.vertices.iter().map(|v| v.position[1]);
        let (lo, hi) = ys.fold((f32::MAX, f32::MIN), |(lo, hi), y| (lo.min(y), hi.max(y)));
        // 44 model pixels at two thirds.
        assert!((lo + 0.5).abs() < 1e-4 && (hi - (44.0 / 24.0 - 0.5)).abs() < 0.01, "{lo} {hi}");
    }
}
