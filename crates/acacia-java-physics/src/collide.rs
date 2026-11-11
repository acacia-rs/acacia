use acacia_registry::prop::{Bottom, StabilityDistance};
use acacia_registry::{Block, BlockState, CollisionBox};

use crate::aabb::Aabb;
use crate::fluid::{fluid_of, Face};
use crate::mth::{floor, horizontal_sqr, length_sqr};

const EPS: f64 = 1.0E-7;
const ABOVE_MARGIN: f64 = 1.0E-5_f32 as f64;
static FULL_BLOCK: [CollisionBox; 1] = [[0.0, 0.0, 0.0, 1.0, 1.0, 1.0]];
static FALLING_SNOW: [CollisionBox; 1] = [[0.0, 0.0, 0.0, 1.0, 0.9_f32 as f64, 1.0]];
static SCAFFOLD_BOTTOM: [CollisionBox; 1] = [[0.0, 0.0, 0.0, 1.0, 0.125, 1.0]];
static LAVA_SURFACE: [CollisionBox; 1] = [[0.0, 0.0, 0.0, 1.0, 0.5, 1.0]];

pub trait Blocks {
    /// The Java state id at a position; `None` when the chunk isn't loaded.
    fn state(&self, x: i32, y: i32, z: i32) -> Option<u32>;

    fn min_y(&self) -> i32;

    fn block_state(&self, pos: [i32; 3]) -> BlockState {
        self.state(pos[0], pos[1], pos[2])
            .and_then(BlockState::from_id)
            .unwrap_or(BlockState::AIR)
    }

    fn has_chunk(&self, pos: [i32; 3]) -> bool {
        self.state(pos[0], pos[1], pos[2]).is_some()
    }
}

/// `EntityCollisionContext`: what scaffolding and powder snow ask about the player.
#[derive(Clone, Copy, Debug, Default)]
pub struct Context {
    pub feet: f64,
    pub descending: bool,
    pub fall_distance: f64,
    pub snow_boots: bool,
    /// A strider: lava sources under open air carry it on their lower half.
    pub stands_on_lava: bool,
}

impl Context {
    fn is_above(&self, y: i32, shape_top: f64) -> bool {
        self.feet > y as f64 + shape_top - ABOVE_MARGIN
    }

    fn boxes(&self, state: BlockState, y: i32) -> Option<&'static [CollisionBox]> {
        match state.block() {
            Block::PowderSnow => Some(self.powder_snow(y)),
            Block::Scaffolding => Some(self.scaffolding(state, y)?),
            _ => state.java_collision(),
        }
    }

    fn powder_snow(&self, y: i32) -> &'static [CollisionBox] {
        if self.fall_distance > 2.5 {
            &FALLING_SNOW
        } else if self.snow_boots && self.is_above(y, 1.0) && !self.descending {
            &FULL_BLOCK
        } else {
            &[]
        }
    }

    fn scaffolding(&self, state: BlockState, y: i32) -> Option<&'static [CollisionBox]> {
        if self.is_above(y, 1.0) && !self.descending {
            return state.java_collision();
        }
        let hanging = state.get(StabilityDistance) != Some(0) && state.get(Bottom) == Some(true);
        Some(if hanging && self.is_above(y, 0.0) { &SCAFFOLD_BOTTOM } else { &[] })
    }
}

/// A rough `isFaceSturdy` from collision boxes: one box must cover the whole face.
pub fn full_face(state: BlockState, face: Face) -> bool {
    let Some(boxes) = state.java_collision() else { return false };
    boxes.iter().any(|b| match face {
        Face::Down => b[1] == 0.0 && b[0] == 0.0 && b[3] == 1.0 && b[2] == 0.0 && b[5] == 1.0,
        Face::North => b[2] == 0.0 && b[0] == 0.0 && b[3] == 1.0 && b[1] == 0.0 && b[4] == 1.0,
        Face::South => b[5] == 1.0 && b[0] == 0.0 && b[3] == 1.0 && b[1] == 0.0 && b[4] == 1.0,
        Face::West => b[0] == 0.0 && b[2] == 0.0 && b[5] == 1.0 && b[1] == 0.0 && b[4] == 1.0,
        Face::East => b[3] == 1.0 && b[2] == 0.0 && b[5] == 1.0 && b[1] == 0.0 && b[4] == 1.0,
    })
}

enum Boxes {
    Block(&'static [CollisionBox]),
    Entity([CollisionBox; 1]),
}

pub struct Shape {
    pub pos: [i32; 3],
    boxes: Boxes,
}

impl Shape {
    fn boxes(&self) -> &[CollisionBox] {
        match &self.boxes {
            Boxes::Block(b) => b,
            Boxes::Entity(b) => b,
        }
    }

    fn offset(&self) -> [f64; 3] {
        self.pos.map(|v| v as f64)
    }

    fn collide(&self, axis: usize, moving: &Aabb, mut distance: f64) -> f64 {
        if distance.abs() < EPS {
            return 0.0;
        }
        let at = self.offset();
        let (b, c) = ((axis + 1) % 3, (axis + 2) % 3);
        for part in self.boxes() {
            let (lo, hi) = part.split_at(3);
            let overlaps = |o: usize| lo[o] + at[o] < moving.max[o] - EPS && hi[o] + at[o] > moving.min[o] + EPS;
            if !overlaps(b) || !overlaps(c) {
                continue;
            }
            if distance > 0.0 {
                let wall = lo[axis] + at[axis] - moving.max[axis];
                if wall >= -EPS {
                    distance = distance.min(wall);
                }
            } else {
                let wall = hi[axis] + at[axis] - moving.min[axis];
                if wall <= EPS {
                    distance = distance.max(wall);
                }
            }
        }
        distance
    }

    fn intersects(&self, area: &Aabb) -> bool {
        let at = self.offset();
        self.boxes().iter().any(|b| {
            let part = Aabb {
                min: [b[0] + at[0], b[1] + at[1], b[2] + at[2]],
                max: [b[3] + at[0], b[4] + at[1], b[5] + at[2]],
            };
            part.intersects(area)
        })
    }

    fn y_coords(&self) -> Vec<f64> {
        let mut ys: Vec<f64> = self.boxes().iter().flat_map(|b| [b[1], b[4]]).collect();
        ys.sort_by(f64::total_cmp);
        ys.dedup();
        ys.iter().map(|y| y + self.pos[1] as f64).collect()
    }
}

/// Everything solid the player runs into, for one collision query.
#[derive(Clone, Copy)]
pub struct Colliders<'a, W: Blocks> {
    pub world: &'a W,
    pub context: Context,
    pub entities: &'a [Aabb],
}

impl<W: Blocks> Colliders<'_, W> {
    /// `LiquidBlock.getCollisionShape` for a strider, else the context's shapes.
    fn boxes(&self, state: BlockState, pos: [i32; 3]) -> Option<&'static [CollisionBox]> {
        let lava_source = |s: BlockState| fluid_of(s).filter(|f| f.lava);
        if self.context.stands_on_lava && lava_source(state).is_some_and(|f| f.source) {
            let above = self.world.block_state([pos[0], pos[1] + 1, pos[2]]);
            let stands = self.context.is_above(pos[1], 0.5) && lava_source(above).is_none();
            return Some(if stands { &LAVA_SURFACE } else { &[] });
        }
        self.context.boxes(state, pos[1])
    }

    /// Block shapes touching `area`, in the order vanilla's cursor visits them: x fastest, then y, then z.
    fn blocks_in(&self, area: &Aabb) -> Vec<Shape> {
        let low = area.min.map(|v| floor(v - EPS) - 1);
        let high = area.max.map(|v| floor(v + EPS) + 1);
        let mut shapes = Vec::new();
        for z in low[2]..=high[2] {
            for y in low[1]..=high[1] {
                for x in low[0]..=high[0] {
                    let state = self.world.state(x, y, z).and_then(BlockState::from_id);
                    let Some(boxes) = state.and_then(|s| self.boxes(s, [x, y, z])) else { continue };
                    let shape = Shape { pos: [x, y, z], boxes: Boxes::Block(boxes) };
                    if !boxes.is_empty() && shape.intersects(area) {
                        shapes.push(shape);
                    }
                }
            }
        }
        shapes
    }

    fn shapes_in(&self, area: &Aabb) -> Vec<Shape> {
        let mut shapes: Vec<Shape> = self
            .entities
            .iter()
            .filter(|e| e.intersects(area))
            .map(|e| Shape {
                pos: [0; 3],
                boxes: Boxes::Entity([[e.min[0], e.min[1], e.min[2], e.max[0], e.max[1], e.max[2]]]),
            })
            .collect();
        shapes.extend(self.blocks_in(area));
        shapes
    }

    pub fn no_collision(&self, area: &Aabb) -> bool {
        self.shapes_in(area).is_empty()
    }

    /// `Entity.collide`: the movement left after collisions, including stepping up.
    pub fn collide(&self, area: Aabb, movement: [f64; 3], on_ground: bool, max_step: f32) -> [f64; 3] {
        let step = if length_sqr(movement) == 0.0 {
            movement
        } else {
            collide_with_shapes(movement, &area, &self.shapes_in(&area.expanded_towards(movement)))
        };
        let blocked = |axis: usize| movement[axis] != step[axis];
        let landed = blocked(1) && movement[1] < 0.0;
        if max_step <= 0.0 || !(landed || on_ground) || !(blocked(0) || blocked(2)) {
            return step;
        }
        let ground = if landed { area.moved([0.0, step[1], 0.0]) } else { area };
        let mut reach = ground.expanded_towards([movement[0], max_step as f64, movement[2]]);
        if !landed {
            reach = reach.expanded_towards([0.0, (-1.0E-5_f32) as f64, 0.0]);
        }
        let shapes = self.shapes_in(&reach);
        for height in step_heights(&ground, &shapes, max_step, step[1] as f32) {
            let stepped = collide_with_shapes([movement[0], height as f64, movement[2]], &ground, &shapes);
            if horizontal_sqr(stepped) > horizontal_sqr(step) {
                return [stepped[0], stepped[1] - (area.min[1] - ground.min[1]), stepped[2]];
            }
        }
        step
    }

    /// `CollisionGetter.findSupportingBlock`: the closest block under `area`, ties to the greater position.
    pub fn supporting_block(&self, area: &Aabb, pos: [f64; 3]) -> Option<[i32; 3]> {
        let mut best: Option<([i32; 3], f64)> = None;
        for shape in self.blocks_in(area) {
            let p = shape.pos;
            let d = (0..3).map(|a| (p[a] as f64 + 0.5 - pos[a]).powi(2)).sum::<f64>();
            let rank = |v: [i32; 3]| (v[1], v[2], v[0]);
            if best.is_none_or(|(q, e)| d < e || (d == e && rank(q) < rank(p))) {
                best = Some((p, d));
            }
        }
        best.map(|(p, _)| p)
    }
}

fn collide_axis(axis: usize, moving: &Aabb, shapes: &[Shape], mut distance: f64) -> f64 {
    for shape in shapes {
        if distance.abs() < EPS {
            return 0.0;
        }
        distance = shape.collide(axis, moving, distance);
    }
    distance
}

fn collide_with_shapes(movement: [f64; 3], area: &Aabb, shapes: &[Shape]) -> [f64; 3] {
    if shapes.is_empty() {
        return movement;
    }
    let order = if movement[0].abs() < movement[2].abs() { [1, 2, 0] } else { [1, 0, 2] };
    let mut resolved = [0.0; 3];
    for axis in order {
        if movement[axis] != 0.0 {
            resolved[axis] = collide_axis(axis, &area.moved(resolved), shapes, movement[axis]);
        }
    }
    resolved
}

fn step_heights(ground: &Aabb, shapes: &[Shape], max_step: f32, skip: f32) -> Vec<f32> {
    let mut candidates = Vec::new();
    for shape in shapes {
        for y in shape.y_coords() {
            let relative = (y - ground.min[1]) as f32;
            if relative < 0.0 || relative == skip {
                continue;
            }
            if relative > max_step {
                break;
            }
            if !candidates.contains(&relative) {
                candidates.push(relative);
            }
        }
    }
    candidates.sort_by(f32::total_cmp);
    candidates
}
