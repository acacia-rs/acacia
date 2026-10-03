//! Faces, reach and aim points.

use acacia_physics::{BlockPos, Vec3};

use crate::state::Entity;

/// Attack/interact reach from the eye along the aim ray. Boar allows 3.005 (to a hitbox grown by
/// 0.1); Java's entity interaction range is 3.0.
pub const ENTITY_REACH: f32 = 3.0;
/// Block reach from the eye to the nearest point of the block. Geyser rejects beyond 5.5 (Java
/// block interaction range 4.5 + 1); vanilla survival reach is 4.5.
pub const BLOCK_REACH: f32 = 4.5;

/// Player hitbox; used for entities missing from [`HITBOXES`].
const ENTITY_WIDTH: f32 = 0.6;
const ENTITY_HEIGHT: f32 = 1.8;
/// Vanilla collision boxes (width, height) of common mounts and mobs, and how far their wire position
/// sits above the box's bottom (BDS: a boat resting on the ground reports y + 0.375). Aiming at a
/// player-sized box over a boat looked past it.
const HITBOXES: &[(&str, f32, f32, f32)] = &[
    ("minecraft:boat", 1.4, 0.455, 0.375),
    ("minecraft:chest_boat", 1.4, 0.455, 0.375),
    ("minecraft:minecart", 0.98, 0.7, 0.0),
    ("minecraft:pig", 0.9, 0.9, 0.0),
    ("minecraft:cow", 0.9, 1.3, 0.0),
    ("minecraft:sheep", 0.9, 1.3, 0.0),
    ("minecraft:chicken", 0.6, 0.8, 0.0),
    ("minecraft:horse", 1.4, 1.6, 0.0),
    ("minecraft:donkey", 1.4, 1.6, 0.0),
    ("minecraft:mule", 1.4, 1.6, 0.0),
    ("minecraft:llama", 0.9, 1.87, 0.0),
    ("minecraft:strider", 0.9, 1.7, 0.0),
    ("minecraft:camel", 1.7, 2.375, 0.0),
];

/// Block face, numbered as on the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Face {
    Down = 0,
    Up = 1,
    /// -Z
    North = 2,
    /// +Z
    South = 3,
    /// -X
    West = 4,
    /// +X
    East = 5,
}

impl Face {
    pub const ALL: [Face; 6] = [Face::Down, Face::Up, Face::North, Face::South, Face::West, Face::East];

    pub fn offset(self) -> BlockPos {
        match self {
            Face::Down => [0, -1, 0],
            Face::Up => [0, 1, 0],
            Face::North => [0, 0, -1],
            Face::South => [0, 0, 1],
            Face::West => [-1, 0, 0],
            Face::East => [1, 0, 0],
        }
    }

    /// The block this face touches: where a block placed against it goes.
    pub fn adjacent(self, pos: BlockPos) -> BlockPos {
        let o = self.offset();
        [pos[0] + o[0], pos[1] + o[1], pos[2] + o[2]]
    }

    /// Centre of the face, relative to the block's minimum corner (the `click_pos` vanilla sends).
    pub fn click_offset(self) -> Vec3 {
        let o = self.offset();
        [0.5 + 0.5 * o[0] as f32, 0.5 + 0.5 * o[1] as f32, 0.5 + 0.5 * o[2] as f32]
    }
}

pub(crate) fn block_min(pos: BlockPos) -> Vec3 {
    [pos[0] as f32, pos[1] as f32, pos[2] as f32]
}

pub(crate) fn block_center(pos: BlockPos) -> Vec3 {
    [pos[0] as f32 + 0.5, pos[1] as f32 + 0.5, pos[2] as f32 + 0.5]
}

/// World position of the centre of `face` of the block at `pos`.
pub(crate) fn face_point(pos: BlockPos, face: Face) -> Vec3 {
    let (m, c) = (block_min(pos), face.click_offset());
    [m[0] + c[0], m[1] + c[1], m[2] + c[2]]
}

/// Distance from `p` to the nearest point of the box.
pub(crate) fn distance_to_box(p: Vec3, min: Vec3, max: Vec3) -> f32 {
    (0..3).map(|i| (min[i] - p[i]).max(p[i] - max[i]).max(0.0).powi(2)).sum::<f32>().sqrt()
}

pub(crate) fn block_distance(eye: Vec3, pos: BlockPos) -> f32 {
    let min = block_min(pos);
    distance_to_box(eye, min, [min[0] + 1.0, min[1] + 1.0, min[2] + 1.0])
}

/// Where the ray from `origin` along `dir` (need not be normalised) enters the box: the distance in
/// units of `dir` and the entry axis. `None` if it misses; `(0, _)` if `origin` is inside.
pub(crate) fn ray_box(origin: Vec3, dir: Vec3, min: Vec3, max: Vec3) -> Option<(f32, usize)> {
    let (mut t_in, mut t_out, mut axis) = (f32::NEG_INFINITY, f32::INFINITY, 1);
    for i in 0..3 {
        if dir[i].abs() < 1e-9 {
            if origin[i] < min[i] || origin[i] > max[i] {
                return None;
            }
            continue;
        }
        let (a, b) = ((min[i] - origin[i]) / dir[i], (max[i] - origin[i]) / dir[i]);
        let (near, far) = if a < b { (a, b) } else { (b, a) };
        if near > t_in {
            (t_in, axis) = (near, i);
        }
        t_out = t_out.min(far);
    }
    (t_in <= t_out && t_out >= 0.0).then_some((t_in.max(0.0), axis))
}

/// The face a ray from `eye` towards the block's centre enters through.
pub fn facing_face(eye: Vec3, pos: BlockPos) -> Face {
    let (min, c) = (block_min(pos), block_center(pos));
    let dir = [c[0] - eye[0], c[1] - eye[1], c[2] - eye[2]];
    match ray_box(eye, dir, min, [min[0] + 1.0, min[1] + 1.0, min[2] + 1.0]) {
        Some((t, axis)) if t > 0.0 => match (axis, dir[axis] > 0.0) {
            (0, true) => Face::West,
            (0, false) => Face::East,
            (1, true) => Face::Down,
            (2, true) => Face::North,
            (2, false) => Face::South,
            _ => Face::Up,
        },
        _ => Face::Up,
    }
}

/// Assumed hitbox of a tracked entity.
pub(crate) fn entity_box(entity: &Entity) -> (Vec3, Vec3) {
    let f = entity.feet();
    let (width, height, lift) = HITBOXES
        .iter()
        .find(|(kind, ..)| *kind == entity.kind)
        .map_or((ENTITY_WIDTH, ENTITY_HEIGHT, 0.0), |&(_, w, h, lift)| (w, h, lift));
    let (h, y) = (width / 2.0, f.y - lift);
    ([f.x - h, y, f.z - h], [f.x + h, y + height, f.z + h])
}

/// Aim point (hitbox centre) and where the ray from `eye` towards it enters the hitbox; the
/// distance to that entry point is what reach checks measure.
pub(crate) fn entity_aim(eye: Vec3, entity: &Entity) -> (Vec3, Vec3, f32) {
    let (min, max) = entity_box(entity);
    let aim = [(min[0] + max[0]) / 2.0, (min[1] + max[1]) / 2.0, (min[2] + max[2]) / 2.0];
    let dir = [aim[0] - eye[0], aim[1] - eye[1], aim[2] - eye[2]];
    let t = ray_box(eye, dir, min, max).map_or(1.0, |(t, _)| t);
    let hit = [eye[0] + dir[0] * t, eye[1] + dir[1] * t, eye[2] + dir[2] * t];
    let distance = (dir[0] * dir[0] + dir[1] * dir[1] + dir[2] * dir[2]).sqrt() * t;
    (aim, hit, distance)
}
