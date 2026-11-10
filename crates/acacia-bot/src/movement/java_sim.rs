//! The Java player ([`acacia_java_physics::Player`]) as planning and following step it. Worlds hand it native ids
//! through the planner's [`Blocks`], which on Java are global palette ids (= neutral state ids).

use acacia_java_physics::{Blocks as JavaBlocks, Keys, Player};
use acacia_physics::{Aabb, BlockPos, Vec3};
use acacia_registry::{Block, BlockState};
use acacia_world::BlockRegistry;

use super::{BEDROCK, Body, Controls, Envelope, Motion, Sim, SimWorld, StepIn, Traits};
use crate::pathfind::{Blocks, Flat, Palette, Terrain};

/// Same as Bedrock's: both editions run the same walk, sprint, jump and gravity numbers.
// TODO: measure hop speed and leap reach with JavaSim instead of borrowing Bedrock's.
pub const JAVA: Envelope = BEDROCK;

const JUMP_DELAY_TICKS: i32 = 10;
/// The overworld's; unloaded chunks below it read as unloaded anyway.
const MIN_Y: i32 = -64;

#[derive(Debug, Clone, PartialEq)]
pub struct JavaSim(pub Player);

fn narrow(v: [f64; 3]) -> Vec3 {
    v.map(|c| c as f32)
}

fn widen(v: Vec3) -> [f64; 3] {
    v.map(f64::from)
}

fn keys(c: &Controls) -> Keys {
    Keys {
        forward: c.forward > 0.0,
        backward: c.forward < 0.0,
        left: c.strafe > 0.0,
        right: c.strafe < 0.0,
        jump: c.jump,
        shift: c.sneak,
        sprint: c.sprint,
    }
}

impl JavaSim {
    /// One client tick with `c` held.
    // TODO: `Controls::glide` and `fly` are not mapped to jump presses yet.
    pub fn step_in(&mut self, c: &Controls, world: &impl JavaBlocks) {
        self.0.yaw = c.yaw;
        self.0.pitch = c.pitch.clamp(-90.0, 90.0);
        self.0.tick(world, keys(c));
    }
}

impl Sim for JavaSim {
    fn envelope() -> &'static Envelope {
        &JAVA
    }

    fn standing(feet: Vec3) -> Self {
        let mut player = Player::at(widen(feet));
        player.on_ground = true;
        JavaSim(player)
    }

    fn body(&self) -> Body {
        let p = &self.0;
        Body {
            pos: narrow(p.pos),
            vel: narrow(p.delta),
            yaw: p.yaw,
            on_ground: p.on_ground,
            h_collision: p.horizontal_collision,
            bonk: p.vertical_collision && !p.on_ground,
            fall: p.fall_distance() as f32,
        }
    }

    fn motion(&self) -> Motion {
        let p = &self.0;
        Motion {
            pos: narrow(p.pos),
            vel: narrow(p.delta),
            yaw: p.yaw,
            on_ground: p.on_ground,
            sprinting: p.sprinting,
            sneaking: p.crouching(),
            pressing_jump: p.keys.jump,
            jump_delay: p.jump_delay().clamp(0, JUMP_DELAY_TICKS) as u8,
        }
    }

    fn synth(&self, m: &Motion) -> Self {
        let base = &self.0;
        let mut p = Player::at(widen(m.pos));
        p.delta = widen(m.vel);
        p.yaw = m.yaw;
        p.on_ground = m.on_ground;
        p.sprinting = m.sprinting;
        p.set_crouching(m.sneaking);
        p.keys = Keys { jump: m.pressing_jump, shift: m.sneaking, ..Keys::default() };
        p.set_jump_delay(i32::from(m.jump_delay));
        p.effects = base.effects.clone();
        p.speed_add = base.speed_add;
        (p.may_fly, p.flying, p.flying_speed) = (base.may_fly, base.flying, base.flying_speed);
        (p.has_glider, p.snow_boots, p.has_food, p.fast_lava) = (base.has_glider, base.snow_boots, base.has_food, base.fast_lava);
        JavaSim(p)
    }

    fn set_motion(&mut self, pos: Vec3, vel: Vec3, yaw: f32) {
        (self.0.pos, self.0.delta, self.0.yaw) = (widen(pos), widen(vel), yaw);
    }

    fn traits(&self) -> Traits {
        let e = &self.0.effects;
        let level = |amp: Option<u8>| amp.map_or(0.0, |a| f32::from(a) + 1.0);
        let multiplier = 1.0 + 0.2 * level(e.speed) - 0.15 * level(e.slowness);
        let speed = (JAVA.movement_speed + self.0.speed_add as f32) * multiplier;
        Traits { speed, jump_boost: e.jump_boost.is_some(), altered_fall: e.levitation.is_some() || e.slow_falling }
    }
}

/// A planner [`Blocks`] holding Java ids, as the Java physics and the follower read it.
pub struct JavaView<'a, B: ?Sized>(pub &'a B);

impl<B: Blocks + ?Sized> JavaBlocks for JavaView<'_, B> {
    fn state(&self, x: i32, y: i32, z: i32) -> Option<u32> {
        self.0.ids([x, y, z]).map(|(id, _)| id)
    }

    fn min_y(&self) -> i32 {
        MIN_Y
    }
}

impl<B: Blocks + ?Sized> JavaView<'_, B> {
    fn neutral(&self, p: BlockPos) -> Option<BlockState> {
        BlockState::from_java_id(self.0.ids(p)?.0)
    }
}

impl<B: Blocks + ?Sized> SimWorld for JavaView<'_, B> {
    fn hazard(&self, p: BlockPos) -> bool {
        self.neutral(p).is_some_and(|s| matches!(s.block(), Block::Lava | Block::Cobweb))
    }

    fn collides(&self, area: &Aabb) -> bool {
        let lo = area.min.map(|v| v.floor() as i32);
        let hi = area.max.map(|v| v.floor() as i32);
        let registry = BlockRegistry::java();
        (lo[0]..=hi[0]).any(|x| {
            (lo[1] - 1..=hi[1]).any(|y| {
                (lo[2]..=hi[2]).any(|z| {
                    let Some((id, _)) = self.0.ids([x, y, z]) else { return false };
                    let boxes = registry.get(id).map_or(&[][..], |s| s.boxes);
                    let at = [x as f32, y as f32, z as f32];
                    boxes.iter().any(|b| (0..3).all(|i| at[i] + b.min[i] < area.max[i] && at[i] + b.max[i] > area.min[i]))
                })
            })
        })
    }

    fn friction(&self, p: BlockPos) -> f32 {
        BlockRegistry::java().get(self.0.ids(p).map_or(0, |(id, _)| id)).map_or(JAVA.friction, |s| s.friction)
    }
}

impl<B: Blocks + ?Sized> StepIn<JavaView<'_, B>> for JavaSim {
    fn step(&mut self, c: &Controls, w: &JavaView<'_, B>) {
        self.step_in(c, w);
    }
}

impl<B: Blocks, P: Palette + ?Sized> StepIn<Terrain<'_, B, P>> for JavaSim {
    fn step(&mut self, c: &Controls, w: &Terrain<'_, B, P>) {
        self.step_in(c, &JavaView(w.blocks()));
    }
}

/// [`Flat`]'s floor as the Java block of that friction.
struct FlatJava(u32);

impl JavaBlocks for FlatJava {
    fn state(&self, _: i32, y: i32, _: i32) -> Option<u32> {
        Some(if y < 0 { self.0 } else { 0 })
    }

    fn min_y(&self) -> i32 {
        MIN_Y
    }
}

fn floor_of(friction: f32) -> Block {
    match friction {
        f if f >= 0.989 => Block::BlueIce,
        f if f >= 0.98 => Block::Ice,
        f if f >= 0.8 => Block::SlimeBlock,
        _ => Block::Stone,
    }
}

impl StepIn<Flat> for JavaSim {
    fn step(&mut self, c: &Controls, w: &Flat) {
        self.step_in(c, &FlatJava(floor_of(w.friction()).default_state().id()));
    }
}
