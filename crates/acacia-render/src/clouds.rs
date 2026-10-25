//! Clouds as 26.3's `CloudRenderer` builds them: the faces of the cloud map's cells within the
//! cloud range, ring by ring outward from the camera's cell, and the scroll that moves them.
//! `clouds.vsh` turns each face into a quad; [`face_quad`] is that, on the CPU.

use glam::{DVec3, Vec3};
use image::RgbaImage;

/// Blocks a cell spans (`CellSize`).
pub const CELL_SIZE: Vec3 = Vec3::new(12.0, 4.0, 12.0);

/// `Direction.get3DDataValue` order, as the shader indexes its tables.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dir {
    Down,
    Up,
    North,
    South,
    West,
    East,
}

const DIRS: [Dir; 6] = [Dir::Down, Dir::Up, Dir::North, Dir::South, Dir::West, Dir::East];
const NORTH_EMPTY: u8 = 8;
const EAST_EMPTY: u8 = 4;
const SOUTH_EMPTY: u8 = 2;
const WEST_EMPTY: u8 = 1;
const CLOUD: u8 = 16;

/// The map's cells: [`CLOUD`] and which neighbours are empty, as `TextureData` packs them.
pub struct CloudMap {
    pub width: i32,
    pub height: i32,
    cells: Vec<u8>,
}

impl CloudMap {
    pub fn new(image: &RgbaImage) -> CloudMap {
        let (width, height) = (image.width() as i32, image.height() as i32);
        let empty = |x: i32, y: i32| image.get_pixel(x as u32, y as u32).0[3] < 10;
        let mut cells = Vec::with_capacity((width * height) as usize);
        for y in 0..height {
            for x in 0..width {
                // East and west wrap by the height, not the width: the game's, kept.
                let flags = [(x, (y - 1).rem_euclid(height), NORTH_EMPTY), ((x + 1).rem_euclid(height), y, EAST_EMPTY), (x, (y + 1).rem_euclid(height), SOUTH_EMPTY), ((x - 1).rem_euclid(height), y, WEST_EMPTY)];
                let cell = if empty(x, y) { 0 } else { flags.iter().fold(CLOUD, |c, &(nx, ny, flag)| if empty(nx, ny) { c | flag } else { c }) };
                cells.push(cell);
            }
        }
        CloudMap { width, height, cells }
    }

    /// 0 for no cloud, else 16 and the empty neighbours (north 8, east 4, south 2, west 1).
    pub fn cell(&self, x: i32, z: i32) -> u8 {
        self.cells[(x.rem_euclid(self.width) + z.rem_euclid(self.height) * self.width) as usize]
    }
}

/// What the renderer is given each frame: the environment's cloud attributes and the options.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CloudLayer {
    /// `visual/cloud_height`.
    pub height: f32,
    /// `visual/cloud_color`, ARGB; no clouds at alpha 0.
    pub color: u32,
    /// Fancy (boxes) or fast (flat) clouds.
    pub fancy: bool,
    /// The Cloud Distance option, in chunks (2..=128, 128 by default).
    pub range: i32,
    /// `FogCloudsEnd`: the range's blocks or `visual/cloud_fog_end_distance`, the nearer.
    pub fog_end: f32,
}

/// `CloudColor` of an ARGB colour (`ARGB.vector4fFromARGB32`), RGBA.
pub fn color(argb: u32) -> [f32; 4] {
    let [a, r, g, b] = argb.to_be_bytes().map(|c| f32::from(c) / 255.0);
    [r, g, b, a]
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelativePos {
    Above,
    Inside,
    Below,
}

/// What `CloudRenderer.prepare` works out each frame. The mesh depends on `cell`, `pos` and the
/// cloud status only.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CloudFrame {
    pub cell: [i32; 2],
    /// `CloudOffset`: the mesh's shift from the camera, in blocks.
    pub offset: Vec3,
    pub pos: RelativePos,
}

/// `bottom_y` is the `cloud_height` attribute.
pub fn frame(map: &CloudMap, bottom_y: f32, camera: DVec3, game_time: i64, partial: f32) -> CloudFrame {
    let rel_bottom = (f64::from(bottom_y) - camera.y) as f32;
    let pos = match () {
        _ if rel_bottom + 4.0 < 0.0 => RelativePos::Above,
        _ if rel_bottom > 0.0 => RelativePos::Below,
        _ => RelativePos::Inside,
    };
    let scroll = (game_time % (i64::from(map.width) * 400)) as f32 + partial;
    let mut x = camera.x + f64::from(scroll * 0.030_000_001);
    let mut z = camera.z + f64::from(3.96f32);
    let (width, height) = (f64::from(map.width) * 12.0, f64::from(map.height) * 12.0);
    x -= (x / width).floor() * width;
    z -= (z / height).floor() * height;
    let cell = [(x / 12.0).floor() as i32, (z / 12.0).floor() as i32];
    let in_cell = [(x - f64::from(cell[0] as f32 * 12.0)) as f32, (z - f64::from(cell[1] as f32 * 12.0)) as f32];
    CloudFrame { cell, offset: Vec3::new(-in_cell[0], rel_bottom, -in_cell[1]), pos }
}

/// Cells around the camera's for a cloud range in chunks.
pub fn radius_cells(range: i32) -> i32 {
    (range as f32 * 16.0 / 12.0).ceil() as i32
}

/// One face; `x`, `z` in cells from the camera's cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Face {
    pub x: i32,
    pub z: i32,
    pub dir: Dir,
    /// Seen from inside the cloud: wound the other way.
    pub inside: bool,
    /// Flat clouds' faces, shaded as tops.
    pub top_color: bool,
}

/// `buildMesh`: fancy clouds are boxes (`extrude`), fast ones a face a cell.
pub fn build_mesh(map: &CloudMap, frame: &CloudFrame, extrude: bool, radius: i32) -> Vec<Face> {
    let mut out = Vec::new();
    let [cx, cz] = frame.cell;
    let mut cell = |x: i32, z: i32| {
        let data = map.cell(cx + x, cz + z);
        if data == 0 {
            return;
        }
        let mut face = |dir, inside, top_color| out.push(Face { x, z, dir, inside, top_color });
        if !extrude {
            return face(Dir::Down, false, true);
        }
        if frame.pos != RelativePos::Below {
            face(Dir::Up, false, false);
        }
        if frame.pos != RelativePos::Above {
            face(Dir::Down, false, false);
        }
        let sides = [(NORTH_EMPTY, z > 0, Dir::North), (SOUTH_EMPTY, z < 0, Dir::South), (WEST_EMPTY, x > 0, Dir::West), (EAST_EMPTY, x < 0, Dir::East)];
        for (flag, facing, dir) in sides {
            if data & flag != 0 && facing {
                face(dir, false, false);
            }
        }
        if x.abs() <= 1 && z.abs() <= 1 {
            DIRS.iter().for_each(|&dir| face(dir, true, false));
        }
    };
    for ring in 0..=2 * radius {
        for x in -ring..=ring {
            let z = ring - x.abs();
            if z >= 0 && z <= radius && x * x + z * z <= radius * radius {
                if z != 0 {
                    cell(x, -z);
                }
                cell(x, z);
            }
        }
    }
    out
}

/// `clouds.vsh`'s unit-cube corners, four a direction.
#[rustfmt::skip]
const CORNERS: [[u8; 3]; 24] = [
    [1, 0, 0], [1, 0, 1], [0, 0, 1], [0, 0, 0],
    [0, 1, 0], [0, 1, 1], [1, 1, 1], [1, 1, 0],
    [0, 0, 0], [0, 1, 0], [1, 1, 0], [1, 0, 0],
    [1, 0, 1], [1, 1, 1], [0, 1, 1], [0, 0, 1],
    [0, 0, 1], [0, 1, 1], [0, 1, 0], [0, 0, 0],
    [1, 0, 0], [1, 1, 0], [1, 1, 1], [1, 0, 1],
];
const SHADES: [f32; 6] = [0.7, 1.0, 0.8, 0.8, 0.9, 0.9];

/// The face's corners before `CloudOffset` is added, and its shade (times `CloudColor`).
pub fn face_quad(face: &Face) -> ([Vec3; 4], f32) {
    let dir = face.dir as usize;
    let cell = Vec3::new(face.x as f32, 0.0, face.z as f32) * CELL_SIZE;
    let corners = std::array::from_fn(|i| {
        let [x, y, z] = CORNERS[dir * 4 + if face.inside { 3 - i } else { i }];
        Vec3::new(f32::from(x), f32::from(y), f32::from(z)) * CELL_SIZE + cell
    });
    (corners, SHADES[if face.top_color { 1 } else { dir }])
}

#[cfg(test)]
#[path = "clouds_tests.rs"]
mod tests;
