use std::collections::HashMap;
use std::sync::Arc;

use acacia_client::proto::packets::ClientboundMapItemData;
use acacia_client::proto::types::{MapDecoration, MapDecorationType};
use acacia_client::proto::{DecodeError, Packet, RawPacket};

/// Pixels along a map's side.
pub const MAP_SIZE: usize = 128;

/// A map's picture: RGBA8, [`MAP_SIZE`] squared, rows top (north) to bottom. Unexplored pixels
/// are transparent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MapImage {
    pub rgba: Vec<u8>,
    pub markers: Vec<MapMarker>,
}

/// A pointer on a map (the players holding it, and others the server adds).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MapMarker {
    /// Half pixels from the picture's middle, east and south.
    pub offset: [i8; 2],
    /// Sixteenths of a turn.
    pub rotation: u8,
    pub kind: MapDecorationType,
}

/// The maps the server has sent pictures of (it sends a held map's unasked), by the id in the
/// item's NBT ([`super::ItemStack::map_id`]). Off by default (see [`crate::BotConfig`]): 64 KB a map.
#[derive(Debug, Default)]
pub struct Maps {
    by_id: HashMap<i64, Arc<MapImage>>,
}

impl Maps {
    pub const PACKETS: &'static [u32] = &[ClientboundMapItemData::ID];

    /// A new `Arc` whenever the picture changes.
    pub fn get(&self, id: i64) -> Option<&Arc<MapImage>> {
        self.by_id.get(&id)
    }

    pub fn apply(&mut self, packet: &RawPacket) -> Result<(), DecodeError> {
        let p: ClientboundMapItemData = packet.decode()?;
        let known = self.by_id.get(&p.map_id);
        let mut image = known.map_or_else(|| MapImage { rgba: vec![0; MAP_SIZE * MAP_SIZE * 4], markers: Vec::new() }, |known| (**known).clone());
        if let Some(decorations) = &p.decorations {
            let marker = |d: &MapDecoration| MapMarker { offset: [d.x as i8, d.y as i8], rotation: d.rotation, kind: d.r#type };
            image.markers = decorations.iter().map(marker).collect();
        }
        paint(&mut image, &p);
        // A packet that says nothing of a map not yet seen makes no blank one.
        let said = p.pixels.is_some() || p.decorations.is_some();
        if known.map_or(said, |known| **known != image) {
            self.by_id.insert(p.map_id, Arc::new(image));
        }
        Ok(())
    }
}

/// Writes the packet's patch of pixels, if it has one that fits.
fn paint(image: &mut MapImage, p: &ClientboundMapItemData) {
    let (Some(pixels), Some(width), Some(height)) = (&p.pixels, p.width, p.height) else { return };
    let size = |n: i32| usize::try_from(n).ok();
    let (Some(width), Some(height), Some(x0), Some(y0)) = (size(width), size(height), size(p.x_offset.unwrap_or(0)), size(p.y_offset.unwrap_or(0))) else {
        return;
    };
    if x0 + width > MAP_SIZE || y0 + height > MAP_SIZE || pixels.len() != width * height {
        return;
    }
    for (index, pixel) in pixels.iter().enumerate() {
        let at = ((y0 + index / width) * MAP_SIZE + x0 + index % width) * 4;
        // Packed with red in the low byte.
        image.rgba[at..at + 4].copy_from_slice(&pixel.to_le_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::queries::test_support::raw;

    fn update(map_id: i64, [x, y]: [i32; 2], [width, height]: [i32; 2], pixels: Vec<i32>) -> RawPacket {
        raw(&ClientboundMapItemData {
            map_id,
            dimension: 0,
            locked: false,
            origin: acacia_client::proto::types::Vec3i { x: 0, y: 0, z: 0 },
            included_in: None,
            scale: None,
            tracked_objects: None,
            decorations: None,
            width: Some(width),
            height: Some(height),
            x_offset: Some(x),
            y_offset: Some(y),
            pixels: Some(pixels),
        })
    }

    #[test]
    fn updates_patch_the_picture_and_bad_ones_are_dropped() {
        let mut maps = Maps::default();
        maps.apply(&update(7, [126, 1], [2, 1], vec![0x0403_0201, -1])).unwrap();
        let first = maps.get(7).unwrap().clone();
        let at = (MAP_SIZE + 126) * 4;
        assert_eq!(first.rgba[at..at + 8], [1, 2, 3, 4, 255, 255, 255, 255]);
        assert!(first.rgba[..at].iter().all(|&b| b == 0));
        // Past the edge, or fewer pixels than the size says.
        maps.apply(&update(7, [127, 0], [2, 1], vec![5, 5])).unwrap();
        maps.apply(&update(7, [0, 0], [2, 2], vec![5])).unwrap();
        assert!(Arc::ptr_eq(&first, maps.get(7).unwrap()));
        maps.apply(&update(7, [0, 0], [1, 1], vec![9])).unwrap();
        assert_eq!((maps.get(7).unwrap().rgba[0], maps.get(7).unwrap().rgba[at]), (9, 1));
        assert!(maps.get(8).is_none());
    }
}
