//! Fancy clouds, as Java's `CloudRenderer` builds them: every opaque texel of the cloud map is a
//! box 12 blocks square and 4 tall, faces between neighbouring cloud cells left out, each face
//! shaded by its side (top 1.0, bottom 0.7, z sides 0.8, x sides 0.9).

use image::RgbaImage;

/// Blocks per cloud-map texel, and a cloud's thickness.
pub const CELL: f32 = 12.0;
pub const THICKNESS: f32 = 4.0;

/// A vertex in cells (x, z) and blocks above the layer's foot (y), with its face's shade.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CloudVertex {
    pub position: [f32; 3],
    pub shade: f32,
}

/// The map's cloud cells as a set, wrapping at its edges like the sky does.
pub struct CloudMap {
    width: i32,
    height: i32,
    cloud: Vec<bool>,
}

impl CloudMap {
    pub fn new(image: &RgbaImage) -> CloudMap {
        let cloud = image.pixels().map(|p| p.0[3] >= 128).collect();
        CloudMap { width: image.width() as i32, height: image.height() as i32, cloud }
    }

    pub fn is_cloud(&self, x: i32, z: i32) -> bool {
        self.cloud[(z.rem_euclid(self.height) * self.width + x.rem_euclid(self.width)) as usize]
    }
}

/// The layer the renderer draws: its world y, and boxes (`fancy`) or Java's fast flat sheet.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CloudLayer {
    pub height: f32,
    pub fancy: bool,
}

/// Java's fast clouds: each cell within `radius` cells of `(cx, cz)` is one face at the layer's
/// foot, in the top's shade.
pub fn sheet(map: &CloudMap, cx: i32, cz: i32, radius: i32) -> Vec<CloudVertex> {
    let cells = (cz - radius..=cz + radius).flat_map(|z| (cx - radius..=cx + radius).map(move |x| (x, z)));
    cells
        .filter(|&(x, z)| map.is_cloud(x, z))
        .flat_map(|(x, z)| {
            let (x0, z0, x1, z1) = (x as f32, z as f32, x as f32 + 1.0, z as f32 + 1.0);
            let v = [[x0, 0.0, z0], [x0, 0.0, z1], [x1, 0.0, z1], [x1, 0.0, z0]].map(|position| CloudVertex { position, shade: 1.0 });
            [v[0], v[1], v[2], v[0], v[2], v[3]]
        })
        .collect()
}

/// The boxes of cells within `radius` cells of `(cx, cz)`, as triangles in cell coordinates.
pub fn boxes(map: &CloudMap, cx: i32, cz: i32, radius: i32) -> Vec<CloudVertex> {
    let mut out = Vec::new();
    for z in cz - radius..=cz + radius {
        for x in cx - radius..=cx + radius {
            if !map.is_cloud(x, z) {
                continue;
            }
            let (x0, z0, x1, z1) = (x as f32, z as f32, x as f32 + 1.0, z as f32 + 1.0);
            let (y0, y1) = (0.0, THICKNESS);
            let mut face = |corners: [[f32; 3]; 4], shade: f32| {
                let v = corners.map(|position| CloudVertex { position, shade });
                out.extend([v[0], v[1], v[2], v[0], v[2], v[3]]);
            };
            face([[x0, y1, z0], [x0, y1, z1], [x1, y1, z1], [x1, y1, z0]], 1.0);
            face([[x0, y0, z0], [x1, y0, z0], [x1, y0, z1], [x0, y0, z1]], 0.7);
            if !map.is_cloud(x, z - 1) {
                face([[x0, y0, z0], [x0, y1, z0], [x1, y1, z0], [x1, y0, z0]], 0.8);
            }
            if !map.is_cloud(x, z + 1) {
                face([[x0, y0, z1], [x1, y0, z1], [x1, y1, z1], [x0, y1, z1]], 0.8);
            }
            if !map.is_cloud(x - 1, z) {
                face([[x0, y0, z0], [x0, y0, z1], [x0, y1, z1], [x0, y1, z0]], 0.9);
            }
            if !map.is_cloud(x + 1, z) {
                face([[x1, y0, z0], [x1, y1, z0], [x1, y1, z1], [x1, y0, z1]], 0.9);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(cells: &[(u32, u32)]) -> CloudMap {
        let mut image = RgbaImage::new(8, 8);
        for &(x, z) in cells {
            image.put_pixel(x, z, image::Rgba([255; 4]));
        }
        CloudMap::new(&image)
    }

    #[test]
    fn a_lone_cell_is_a_closed_box_and_neighbours_share_no_wall() {
        assert_eq!(boxes(&map(&[(3, 3)]), 3, 3, 2).len(), 6 * 6);
        // Two cells side by side: 2 × (top + bottom) + 6 outer walls.
        assert_eq!(boxes(&map(&[(3, 3), (4, 3)]), 3, 3, 2).len(), (4 + 6) * 6);
    }

    #[test]
    fn fast_clouds_are_one_flat_face_a_cell() {
        let flat = sheet(&map(&[(3, 3), (4, 3)]), 3, 3, 2);
        assert_eq!(flat.len(), 2 * 6);
        assert!(flat.iter().all(|v| v.position[1] == 0.0 && v.shade == 1.0));
    }

    #[test]
    fn the_map_wraps() {
        let m = map(&[(0, 0)]);
        assert!(m.is_cloud(8, -8) && !m.is_cloud(1, 0));
        // The cell one map-width away is the same cloud.
        assert_eq!(boxes(&m, 8, 0, 0).len(), 6 * 6);
    }
}
