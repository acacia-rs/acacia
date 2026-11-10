use acacia_registry::prop::{Level, Waterlogged};
use acacia_registry::{Block, BlockState, BlockTag};

use crate::aabb::Aabb;
use crate::collide::{full_face, Blocks};
use crate::mth::{ceil, floor, normalize};
use crate::player::Player;

const HORIZONTAL: [([i32; 3], Face); 4] = [
    ([0, 0, -1], Face::North),
    ([1, 0, 0], Face::East),
    ([0, 0, 1], Face::South),
    ([-1, 0, 0], Face::West),
];
const WATER_CURRENT: f64 = 0.014;
const LAVA_CURRENT: f64 = 0.0023333333333333335;
const FAST_LAVA_CURRENT: f64 = 0.007;
const MIN_IMPULSE: f64 = 0.0045000000000000005;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Face {
    Down,
    North,
    East,
    South,
    West,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fluid {
    pub lava: bool,
    pub source: bool,
    pub amount: u8,
    pub falling: bool,
}

impl Fluid {
    fn own_height(self) -> f32 {
        self.amount as f32 / 9.0
    }

    /// Vanilla keeps one tracker per fluid holder, so still and flowing water have separate heights.
    fn tracker(self) -> usize {
        (self.lava as usize) * 2 + self.source as usize
    }
}

const SOURCE_WATER: Fluid = Fluid { lava: false, source: true, amount: 8, falling: false };

fn from_level(lava: bool, level: u8) -> Fluid {
    match level {
        0 => Fluid { lava, source: true, amount: 8, falling: false },
        1..=7 => Fluid { lava, source: false, amount: 8 - level, falling: false },
        _ => Fluid { lava, source: false, amount: 8, falling: true },
    }
}

pub fn fluid_of(state: BlockState) -> Option<Fluid> {
    match state.block() {
        Block::Water => return Some(from_level(false, state.get(Level).unwrap_or(0))),
        Block::Lava => return Some(from_level(true, state.get(Level).unwrap_or(0))),
        Block::BubbleColumn | Block::Kelp | Block::KelpPlant | Block::Seagrass | Block::TallSeagrass => return Some(SOURCE_WATER),
        _ => {}
    }
    (state.get(Waterlogged) == Some(true)).then_some(SOURCE_WATER)
}

fn fluid_at(world: &impl Blocks, pos: [i32; 3]) -> Option<Fluid> {
    fluid_of(world.block_state(pos))
}

pub fn contains_any_liquid(world: &impl Blocks, area: &Aabb) -> bool {
    let low = area.min.map(floor);
    let high = area.max.map(ceil);
    (low[0]..high[0]).any(|x| (low[1]..high[1]).any(|y| (low[2]..high[2]).any(|z| fluid_at(world, [x, y, z]).is_some())))
}

fn offset(pos: [i32; 3], by: [i32; 3]) -> [i32; 3] {
    [pos[0] + by[0], pos[1] + by[1], pos[2] + by[2]]
}

fn height(world: &impl Blocks, pos: [i32; 3], fluid: Fluid) -> f32 {
    let above = fluid_at(world, offset(pos, [0, 1, 0]));
    if above.is_some_and(|a| a.lava == fluid.lava) { 1.0 } else { fluid.own_height() }
}

fn camera_height(world: &impl Blocks, pos: [i32; 3], fluid: Fluid) -> f32 {
    if fluid.source && full_face(world.block_state(offset(pos, [0, 1, 0])), Face::Down) {
        return 1.0;
    }
    height(world, pos, fluid)
}

fn solid_face(world: &impl Blocks, pos: [i32; 3], face: Face, lava: bool) -> bool {
    let state = world.block_state(pos);
    if fluid_of(state).is_some_and(|f| f.lava == lava) || matches!(state.block(), Block::Ice | Block::FrostedIce) {
        return false;
    }
    full_face(state, face)
}

fn flow(world: &impl Blocks, pos: [i32; 3], fluid: Fluid) -> [f64; 3] {
    let affects = |f: Option<Fluid>| f.is_none_or(|f| f.lava == fluid.lava);
    let (mut flow_x, mut flow_z) = (0.0_f64, 0.0_f64);
    for (step, _) in HORIZONTAL {
        let side = offset(pos, step);
        let neighbour = fluid_at(world, side);
        if !affects(neighbour) {
            continue;
        }
        let side_height = neighbour.map_or(0.0, Fluid::own_height);
        let mut distance = 0.0_f32;
        if side_height == 0.0 {
            if !world.block_state(side).block().is(BlockTag::BlocksFluidFlow) {
                let below = fluid_at(world, offset(side, [0, -1, 0]));
                if affects(below) {
                    let below_height = below.map_or(0.0, Fluid::own_height);
                    if below_height > 0.0 {
                        distance = fluid.own_height() - (below_height - 0.8888889);
                    }
                }
            }
        } else if side_height > 0.0 {
            distance = fluid.own_height() - side_height;
        }
        if distance != 0.0 {
            flow_x += (step[0] as f32 * distance) as f64;
            flow_z += (step[2] as f32 * distance) as f64;
        }
    }
    let mut current = [flow_x, 0.0, flow_z];
    if fluid.falling {
        for (step, face) in HORIZONTAL {
            let side = offset(pos, step);
            if solid_face(world, side, face, fluid.lava) || solid_face(world, offset(side, [0, 1, 0]), face, fluid.lava) {
                current = normalize(current);
                current[1] += -6.0;
                break;
            }
        }
    }
    normalize(current)
}

#[derive(Clone, Copy, Debug, Default)]
struct Accumulator {
    height: f64,
    total: [f64; 3],
    count: u32,
}

impl Accumulator {
    fn impulse(&self, delta: [f64; 3], scale: f64) -> Option<[f64; 3]> {
        let t = self.total;
        if self.count == 0 || t[0] * t[0] + t[1] * t[1] + t[2] * t[2] < 1.0E-5_f32 as f64 {
            return None;
        }
        let share = 1.0 / self.count as f64;
        let mut impulse = t.map(|v| v * share * scale);
        let length = (impulse[0] * impulse[0] + impulse[1] * impulse[1] + impulse[2] * impulse[2]).sqrt();
        if delta[0].abs() < 0.003 && delta[2].abs() < 0.003 && length < MIN_IMPULSE {
            impulse = normalize(impulse).map(|v| v * MIN_IMPULSE);
        }
        Some(impulse)
    }
}

/// What the last `updateFluidInteraction` saw.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FluidContact {
    heights: [f64; 4],
    eyes: [bool; 4],
}

impl FluidContact {
    pub fn height(&self, lava: bool) -> f64 {
        let (a, b) = if lava { (self.heights[2], self.heights[3]) } else { (self.heights[0], self.heights[1]) };
        a.max(b).max(0.0)
    }

    pub fn eyes_in_water(&self) -> bool {
        self.eyes[0] || self.eyes[1]
    }
}

impl Player {
    fn chunks_loaded(&self, world: &impl Blocks, low: [i32; 3], high: [i32; 3]) -> bool {
        [low[0], high[0]].iter().all(|&x| [low[2], high[2]].iter().all(|&z| world.has_chunk([x, low[1], z])))
    }

    /// `Entity.updateFluidInteraction`, including the push from currents.
    pub(crate) fn update_fluids(&mut self, world: &impl Blocks) {
        self.fluids = FluidContact::default();
        let area = self.bounding_box().deflated(0.001);
        let low = area.min.map(floor);
        let high = area.max.map(|v| ceil(v) - 1);
        let mut currents = [Accumulator::default(); 2];
        if self.chunks_loaded(world, [low[0] - 1, low[1], low[2] - 1], [high[0] + 1, high[1], high[2] + 1]) {
            let (entity_y, eye_y, here) = (self.bounding_box().min[1], self.eye_y(), self.block_pos());
            let pushed = !self.flying;
            for x in low[0]..=high[0] {
                for y in low[1]..=high[1] {
                    for z in low[2]..=high[2] {
                        let pos = [x, y, z];
                        let Some(fluid) = fluid_at(world, pos) else { continue };
                        let bottom = y as f64;
                        let top = bottom + height(world, pos, fluid) as f64;
                        if top < area.min[1] {
                            continue;
                        }
                        let tracker = fluid.tracker();
                        if x == here[0] && z == here[2] && eye_y >= bottom && eye_y <= bottom + camera_height(world, pos, fluid) as f64 {
                            self.fluids.eyes[tracker] = true;
                        }
                        self.fluids.heights[tracker] = (top - entity_y).max(self.fluids.heights[tracker]);
                        if pushed {
                            let current = &mut currents[fluid.lava as usize];
                            let mut push = flow(world, pos, fluid);
                            current.height = self.fluids.heights[tracker].max(current.height);
                            if current.height < 0.4 {
                                push = push.map(|v| v * current.height);
                            }
                            for axis in 0..3 {
                                current.total[axis] += push[axis];
                            }
                            current.count += 1;
                        }
                    }
                }
            }
        }
        let in_water = self.fluids.height(false) > 0.0;
        let in_lava = self.fluids.height(true) > 0.0;
        if in_water {
            self.fall_distance = 0.0;
        }
        self.in_water = in_water;
        if !self.flying {
            if in_water {
                self.add_current(currents[0], WATER_CURRENT);
            }
            if in_lava {
                self.add_current(currents[1], if self.fast_lava { FAST_LAVA_CURRENT } else { LAVA_CURRENT });
            }
        }
    }

    fn add_current(&mut self, current: Accumulator, scale: f64) {
        if let Some(impulse) = current.impulse(self.delta, scale) {
            for axis in 0..3 {
                self.delta[axis] += impulse[axis];
            }
        }
    }

    pub(crate) fn water_at_block(&self, world: &impl Blocks) -> bool {
        fluid_at(world, self.block_pos()).is_some_and(|f| !f.lava)
    }

    pub(crate) fn fluid_above_feet(&self, world: &impl Blocks) -> bool {
        let pos = [floor(self.pos[0]), floor(self.pos[1] + 1.0 - 0.1), floor(self.pos[2])];
        fluid_at(world, pos).is_some()
    }
}
