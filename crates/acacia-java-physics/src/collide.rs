use acacia_registry::{BlockState, CollisionBox};

use crate::aabb::Aabb;
use crate::mth::{floor, horizontal_sqr};

const EPS: f64 = 1.0E-7;

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

pub struct Shape {
    pub pos: [i32; 3],
    boxes: &'static [CollisionBox],
}

impl Shape {
    fn offset(&self) -> [f64; 3] {
        self.pos.map(|v| v as f64)
    }

    fn collide(&self, axis: usize, moving: &Aabb, mut distance: f64) -> f64 {
        if distance.abs() < EPS {
            return 0.0;
        }
        let at = self.offset();
        let (b, c) = ((axis + 1) % 3, (axis + 2) % 3);
        for shape_box in self.boxes {
            let (lo, hi) = (shape_box.split_at(3).0, shape_box.split_at(3).1);
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
        self.boxes.iter().any(|b| {
            let shape_box = Aabb {
                min: [b[0] + at[0], b[1] + at[1], b[2] + at[2]],
                max: [b[3] + at[0], b[4] + at[1], b[5] + at[2]],
            };
            shape_box.intersects(area)
        })
    }

    fn y_coords(&self) -> Vec<f64> {
        let mut ys: Vec<f64> = self.boxes.iter().flat_map(|b| [b[1], b[4]]).collect();
        ys.sort_by(f64::total_cmp);
        ys.dedup();
        ys.iter().map(|y| y + self.pos[1] as f64).collect()
    }
}

/// Block shapes touching `area`, in the order vanilla's cursor visits them: x fastest, then y, then z.
pub fn shapes_in(world: &impl Blocks, area: &Aabb) -> Vec<Shape> {
    let low = area.min.map(|v| floor(v - EPS) - 1);
    let high = area.max.map(|v| floor(v + EPS) + 1);
    let mut shapes = Vec::new();
    for z in low[2]..=high[2] {
        for y in low[1]..=high[1] {
            for x in low[0]..=high[0] {
                let Some(boxes) = world.state(x, y, z).and_then(BlockState::from_id).and_then(BlockState::java_collision) else {
                    continue;
                };
                let shape = Shape { pos: [x, y, z], boxes };
                if !boxes.is_empty() && shape.intersects(area) {
                    shapes.push(shape);
                }
            }
        }
    }
    shapes
}

pub fn no_collision(world: &impl Blocks, area: &Aabb) -> bool {
    shapes_in(world, area).is_empty()
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

/// `Entity.collide`: the movement left after blocks, including stepping up.
pub fn collide(world: &impl Blocks, area: Aabb, movement: [f64; 3], on_ground: bool, max_step: f32) -> [f64; 3] {
    let step = if crate::mth::length_sqr(movement) == 0.0 {
        movement
    } else {
        collide_with_shapes(movement, &area, &shapes_in(world, &area.expanded_towards(movement)))
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
    let shapes = shapes_in(world, &reach);
    for height in step_heights(&ground, &shapes, max_step, step[1] as f32) {
        let stepped = collide_with_shapes([movement[0], height as f64, movement[2]], &ground, &shapes);
        if horizontal_sqr(stepped) > horizontal_sqr(step) {
            return [stepped[0], stepped[1] - (area.min[1] - ground.min[1]), stepped[2]];
        }
    }
    step
}

/// `CollisionGetter.findSupportingBlock`: the closest block under `area`, ties to the greater position.
pub fn supporting_block(world: &impl Blocks, area: &Aabb, pos: [f64; 3]) -> Option<[i32; 3]> {
    let mut best: Option<([i32; 3], f64)> = None;
    for shape in shapes_in(world, area) {
        let p = shape.pos;
        let d = (0..3).map(|a| (p[a] as f64 + 0.5 - pos[a]).powi(2)).sum::<f64>();
        let rank = |v: [i32; 3]| (v[1], v[2], v[0]);
        if best.is_none_or(|(q, e)| d < e || (d == e && rank(q) < rank(p))) {
            best = Some((p, d));
        }
    }
    best.map(|(p, _)| p)
}
