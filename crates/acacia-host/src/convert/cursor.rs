use acacia_api::{ApiError, BlockAtCursorParams, BlockAtCursorResult, BlockPos, CursorHit, ErrorCode, Face, Vec3};
use acacia_bot::snapshot::SnapshotReader;
use acacia_bot::state::PlayerState;
use acacia_world::ray::{self, RayHit};
use acacia_world::{selection, Aabb, BlockAccess, World};

use super::world::block;

const DEFAULT_DISTANCE: f32 = 6.0;
const MAX_DISTANCE: f32 = 64.0;

/// A world's loaded chunks as `BlockAccess`; unloaded cells read as air.
struct Loaded<'a>(&'a World);

impl BlockAccess for Loaded<'_> {
    fn block(&self, x: i32, y: i32, z: i32) -> u32 {
        self.0.get(x >> 4, z >> 4).map_or(self.0.dimension().air, |c| c.read().block(x, y, z))
    }

    fn liquid(&self, x: i32, y: i32, z: i32) -> u32 {
        self.0.get(x >> 4, z >> 4).map_or(self.0.dimension().air, |c| c.read().liquid(x, y, z))
    }
}

/// `bot.blockAtCursor` from the bot's published eyes and rotation.
pub(crate) fn block_at_cursor(reader: &SnapshotReader, p: &BlockAtCursorParams) -> Result<BlockAtCursorResult, ApiError> {
    let world = reader.world().ok_or_else(|| ApiError::new(ErrorCode::NotPossible, "the bot tracks no terrain yet"))?;
    let hot = reader.hot();
    // The standing eye height: the hot snapshot has no pose, so a sneaking bot's ray starts 0.35 high.
    let eye = [hot.position[0], hot.position[1] + PlayerState::EYE_HEIGHT, hot.position[2]].map(f64::from);
    let (ys, yc) = hot.yaw.to_radians().sin_cos();
    let (ps, pc) = hot.pitch.to_radians().sin_cos();
    let dir = [-ys * pc, -ps, yc * pc].map(f64::from);
    let reach = f64::from(p.max_distance.unwrap_or(DEFAULT_DISTANCE).clamp(0.0, MAX_DISTANCE));
    let (cells, registry) = (Loaded(&world), world.registry());
    // As acacia-bot's sight: collision stops an anticheat's ray, selection the crosshair's.
    let shapes = |pos: [i32; 3], id: u32, out: &mut Vec<Aabb>| {
        if let Some(s) = registry.get(id) {
            out.extend(s.boxes.iter().copied().chain(selection::boxes_at(&cells, registry, pos)));
        }
    };
    let hit = ray::cast(&cells, eye, dir, reach, shapes).and_then(|h| cursor_hit(&world, h));
    Ok(BlockAtCursorResult { hit })
}

fn cursor_hit(world: &World, h: RayHit) -> Option<CursorHit> {
    let position = BlockPos::from(h.block);
    Some(CursorHit {
        block: block(world, position)?,
        position,
        face: wire_face(h.face),
        point: Vec3::from(h.point.map(|c| c as f32)),
        distance: h.distance as f32,
    })
}

fn wire_face(face: u8) -> Face {
    match face {
        0 => Face::Down,
        1 => Face::Up,
        2 => Face::North,
        3 => Face::South,
        4 => Face::West,
        _ => Face::East,
    }
}
