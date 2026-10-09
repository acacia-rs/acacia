//! A player-paddled boat, as BDS 1.26.52 moves one (docs/research/riding-fishing-elytra.md "Boat"):
//! friction, the paddles' thrust and torque, the move, then gravity and buoyancy for the next tick.
//! The server's big waves are random; [`wave_phases`] reads the wave back out of a correction.

use std::f64::consts::PI;

use crate::aabb::Aabb;
use crate::clip::clip_all;
use crate::math::Vec3;
use crate::world::{BlockPhysics, LiquidKind, WorldView};

const HALF_WIDTH: f32 = 0.7;
const HEIGHT: f32 = 0.455;
/// The position is this far above the bottom of the box.
const HEIGHT_OFFSET: f32 = 0.375;
/// Speed a paddle adds per unit of its force.
const THRUST: f32 = 0.01375;
/// A stroke pulls at 3.0 on its first tick and 2.9 on the other nine.
const STROKE_TICKS: u8 = 10;
const WATER_FRICTION: f32 = 0.9;
/// Squared speed below which a turning boat turns faster and pushes less.
const SLOW_SQUARED: f32 = 0.010000001;
const RADIANS: f32 = 0.017453292;
const WAVE_HEIGHT: f32 = 0.035;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoatState {
    /// The wire position.
    pub pos: Vec3,
    /// Motion per tick: x and z as last moved, y for the next move.
    pub vel: Vec3,
    /// Degrees, never wrapped; the bow points 90° left of it.
    pub yaw: f32,
    /// Degrees per tick.
    pub yaw_vel: f32,
    pub on_ground: bool,
    /// Ticks into the left and right paddle's stroke; `None` at rest.
    strokes: [Option<u8>; 2],
    /// The wave's phase in radians.
    pub wave: f64,
}

impl BoatState {
    pub fn new(pos: Vec3, yaw: f32) -> Self {
        BoatState { pos, vel: [0.0; 3], yaw, yaw_vel: 0.0, on_ground: false, strokes: [None; 2], wave: 0.0 }
    }

    fn bounding_box(&self) -> Aabb {
        let [x, y, z] = self.pos;
        let bottom = y - HEIGHT_OFFSET;
        Aabb::new(x - HALF_WIDTH, bottom, z - HALF_WIDTH, x + HALF_WIDTH, bottom + HEIGHT, z + HALF_WIDTH)
    }
}

/// One tick of a boat whose driver holds `keys` ([strafe, forward], each -1, 0 or 1; strafe + is left).
pub fn boat_tick<W: WorldView + ?Sized>(st: &mut BoatState, keys: [f32; 2], world: &W) {
    let [left, right] = paddles(keys);
    let forces = [stroke(left, &mut st.strokes[0]) * THRUST, stroke(right, &mut st.strokes[1]) * THRUST];
    let friction = friction(st, world);
    let [mut vx, vy, mut vz] = st.vel;
    let (fr, mut forward, mut torque) = match friction {
        Some(fr) => (fr, forces[0] + forces[1], forces[0] * 3.0 + forces[1] * -3.0),
        // In the air nothing slows the boat and the paddles find no hold.
        None => (1.0, 0.0, 0.0),
    };
    (vx, vz, st.yaw_vel) = (vx * fr, vz * fr, st.yaw_vel * fr);
    if vz * vz + vx * vx < SLOW_SQUARED && torque != 0.0 {
        forward *= fr;
        torque *= 1.6;
    }
    st.yaw_vel = (torque * 10.0 + st.yaw_vel) * fr;
    st.yaw += st.yaw_vel;
    let heading = (90.0 - st.yaw) * RADIANS;
    vx = (forward * heading.sin() + vx) * fr;
    vz = (forward * heading.cos() + vz) * fr;
    st.vel = slide(st, [vx, vy, vz], world);
    st.vel[1] = float(st, world);
}

/// The pull of the left and right paddle, -0.15 to 1.
fn paddles([strafe, forward]: [f32; 2]) -> [f32; 2] {
    // The move vector is normalised, but a held forward key counts in full (W+A, live 2026-10-09).
    let length = (strafe * strafe + forward * forward).sqrt().max(1.0);
    let (mut x, y) = (strafe / length, if forward > 0.0 { 1.0 } else { forward / length });
    let mut pull = (y * y + x * x).sqrt();
    if y < 0.0 {
        (x, pull) = (-x, pull * -0.15);
    }
    let left = if x > 1.0 { 0.0 } else if x > 0.0 { 1.0 - x } else { 1.0 };
    let right = if x <= 0.0 { x.max(-1.0) + 1.0 } else { 1.0 };
    [left * pull, right * pull]
}

fn stroke(pull: f32, since: &mut Option<u8>) -> f32 {
    if pull == 0.0 {
        *since = None;
        return 0.0;
    }
    match since {
        Some(ticks) if *ticks + 1 < STROKE_TICKS => {
            *ticks += 1;
            pull.signum() * ((3.0 * pull).abs() - 0.1).max(0.0)
        }
        _ => {
            *since = Some(0);
            3.0 * pull
        }
    }
}

fn water(block: &BlockPhysics) -> bool {
    block.liquid.is_some_and(|l| l.kind == LiquidKind::Water)
}

fn cell(pos: Vec3) -> [i32; 3] {
    pos.map(|c| c.floor() as i32)
}

/// Floating at the surface: in a water block with none above, below its top.
fn at_surface<W: WorldView + ?Sized>(pos: Vec3, world: &W) -> bool {
    let [x, y, z] = cell(pos);
    let here = world.block([x, y, z]);
    let Some(liquid) = here.liquid.filter(|_| water(&here)) else { return false };
    // Bedrock's `liquid_depth`: 0 a source, 8 and up falling.
    let depth = if liquid.falling { 8 } else { 8 - i32::from(liquid.depth) };
    let top = (y + 1) as f32 - (if depth < 8 { (depth + 1) as f32 } else { 1.0 } / 9.0 - 0.1111111);
    pos[1] < top && !water(&world.block([x, y + 1, z]))
}

fn submerged<W: WorldView + ?Sized>(pos: Vec3, world: &W) -> bool {
    let [x, y, z] = cell(pos);
    water(&world.block([x, y, z])) && water(&world.block([x, y + 1, z]))
}

/// What slows the boat this tick; `None` in the air.
fn friction<W: WorldView + ?Sized>(st: &BoatState, world: &W) -> Option<f32> {
    if at_surface(st.pos, world) || submerged(st.pos, world) {
        return Some(WATER_FRICTION);
    }
    let [x, y, z] = cell(st.pos);
    let (here, below) = (world.block([x, y, z]), world.block([x, y - 1, z]));
    if water(&here) || (here.air && !st.on_ground && water(&below)) {
        return Some(WATER_FRICTION);
    }
    st.on_ground.then_some(if here.air { below.friction } else { here.friction })
}

/// Moves by `vel` against the blocks, Y then X then Z, and returns the motion that was left.
fn slide<W: WorldView + ?Sized>(st: &mut BoatState, vel: Vec3, world: &W) -> Vec3 {
    let mut bb = st.bounding_box();
    let mut boxes = Vec::new();
    world.collisions(&bb.extend(vel), &mut boxes);
    let mut moved = [0.0; 3];
    for axis in [1, 0, 2] {
        let mut step = [0.0; 3];
        step[axis] = vel[axis];
        let step = clip_all(&boxes, &bb, step, true, None);
        bb = bb.translate(step);
        moved[axis] = step[axis];
    }
    st.on_ground = vel[1] < 0.0 && moved[1] != vel[1];
    for axis in 0..3 {
        st.pos[axis] += moved[axis];
    }
    moved
}

/// How far the wave moves on in a tick that leaves the boat with `vel`, big waves aside.
pub fn wave_step(vel: Vec3) -> f64 {
    f64::from(((vel[2] * vel[2] + vel[0] * vel[0]).sqrt() * 30.0 + 1.0) * 0.05)
}

/// How far a floating boat at height `y` sits under the surface, 0.1 to 1.
fn depth(y: f32) -> f32 {
    ((1.0 - (y - y.floor())) * 0.9 + 0.1).clamp(0.0, 1.0)
}

/// Advances the wave and returns the vertical motion for the next tick: gravity, then the lift
/// of the water towards the wave's height.
fn float<W: WorldView + ?Sized>(st: &mut BoatState, world: &W) -> f32 {
    st.wave += wave_step(st.vel);
    let vy = (st.vel[1] - 0.04) * 0.98;
    let (depth, wave) = if at_surface(st.pos, world) {
        (depth(st.pos[1]), (st.wave.sin() as f32 + 1.0) * WAVE_HEIGHT)
    } else if submerged(st.pos, world) {
        (1.0, 0.0)
    } else {
        return vy;
    };
    (vy * 0.7 + 0.05).min(((depth - wave) - 0.1) * 0.15)
}

/// The two wave phases (radians) under which a boat floating at `st.pos` gets `st.vel`'s vertical
/// motion. `None` when the water's lift was not what set the motion.
pub fn wave_phases(st: &BoatState) -> Option<[f64; 2]> {
    let wave = depth(st.pos[1]) - 0.1 - st.vel[1] / 0.15;
    let sine = f64::from(wave / WAVE_HEIGHT - 1.0);
    (sine.abs() <= 1.0001).then(|| {
        let phase = sine.clamp(-1.0, 1.0).asin();
        [phase, PI - phase]
    })
}
