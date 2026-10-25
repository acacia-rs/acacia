//! Where the camera holds an item, in view space (x right, y up, -z ahead), before the item's
//! `firstperson_*hand` display: `FirstPersonHandsAndItemsRenderer.submitArmWithItem` for every
//! item but maps, arm swing and use animations included. Checked by the parity harness
//! `item_transforms`.

use glam::{Mat4, Vec3};

use crate::entity::java::mth;

/// The item in use in this hand; `held` is Java's `timeHeld`, ticks since the use began.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Using {
    /// Food or drink taking `duration` ticks.
    Eat { held: f32, duration: f32 },
    /// A sword or other non-shield item raised to block (a shield's block is its display's).
    Block,
    Bow { held: f32 },
    Trident { held: f32 },
    /// A crossbow being loaded, `charge` ticks to full.
    Crossbow { held: f32, charge: f32 },
}

/// One hand's state.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Hold {
    pub left: bool,
    pub main_hand: bool,
    /// Java's `inverseArmHeight`: 0 when the item is fully up, 1 when it is lowered out of view.
    pub lowered: f32,
    /// The swing's progress, 0 to 1; Java's `attack`.
    pub attack: f32,
    /// This hand is swinging (`currentSwing` is its hand and a WHACK).
    pub swinging: bool,
    pub using: Option<Using>,
    /// The item is a crossbow; `Some(true)` charged.
    pub crossbow: Option<bool>,
    /// A riptide spin attack.
    pub spinning: bool,
}

fn rot(axis: Vec3, degrees: f32) -> Mat4 {
    Mat4::from_axis_angle(axis, degrees.to_radians())
}

fn arm(side: f32, lowered: f32) -> Mat4 {
    Mat4::from_translation(Vec3::new(side * 0.56, -0.52 + lowered * -0.6, -0.72))
}

/// `applyItemArmAttackTransform`.
fn attack_turn(side: f32, attack: f32) -> Mat4 {
    let y_swing = mth::sin(attack * attack * std::f32::consts::PI);
    let xz_swing = mth::sin(attack.sqrt() * std::f32::consts::PI);
    rot(Vec3::Y, side * (45.0 + y_swing * -20.0)) * rot(Vec3::Z, side * xz_swing * -20.0) * rot(Vec3::X, xz_swing * -80.0) * rot(Vec3::Y, side * -45.0)
}

/// `swingArm`.
fn swing(side: f32, attack: f32) -> Mat4 {
    let pi = std::f32::consts::PI;
    let x = -0.4 * mth::sin(attack.sqrt() * pi);
    let y = 0.2 * mth::sin(attack.sqrt() * (pi * 2.0));
    let z = -0.2 * mth::sin(attack * pi);
    Mat4::from_translation(Vec3::new(side * x, y, z)) * attack_turn(side, attack)
}

/// The pull of a drawn bow, trident or crossbow: its shake, push and stretch, then the turn back.
fn drawn(side: f32, held: f32, power: f32, push: f32) -> Mat4 {
    let shake = if power > 0.1 { mth::sin((held - 0.1) * 1.3) * (power - 0.1) } else { 0.0 };
    Mat4::from_translation(Vec3::new(0.0, shake * 0.004, 0.0))
        * Mat4::from_translation(Vec3::new(0.0, 0.0, power * push))
        * Mat4::from_scale(Vec3::new(1.0, 1.0, 1.0 + power * 0.2))
        * rot(Vec3::NEG_Y, side * 45.0)
}

/// `applyEatTransform`: `held` ticks into a `duration`-tick use.
fn eat(side: f32, held: f32, duration: f32) -> Mat4 {
    let current = duration - held;
    let scaled = current / duration;
    let bob = if scaled < 0.8 { (mth::cos(current / 4.0 * std::f32::consts::PI) * 0.1).abs() } else { 0.0 };
    let jiggle = 1.0 - scaled.powi(27);
    Mat4::from_translation(Vec3::new(0.0, bob, 0.0))
        * Mat4::from_translation(Vec3::new(jiggle * 0.6 * side, jiggle * -0.5, 0.0))
        * rot(Vec3::Y, side * jiggle * 90.0)
        * rot(Vec3::X, jiggle * 10.0)
        * rot(Vec3::Z, side * jiggle * 30.0)
}

/// View space to the space the item's display starts from.
pub fn frame(h: &Hold) -> Mat4 {
    let side = if h.left { -1.0 } else { 1.0 };
    let arm = arm(side, h.lowered);
    if let Some(charged) = h.crossbow {
        if let (Some(Using::Crossbow { held, charge }), false) = (h.using, charged) {
            let turn = Mat4::from_translation(Vec3::new(side * -0.478_568_2, -0.094_387, 0.057_315_31)) * rot(Vec3::X, -11.935) * rot(Vec3::Y, side * 65.3) * rot(Vec3::Z, side * -9.785);
            return arm * turn * drawn(side, held, (held / charge).min(1.0), 0.04);
        }
        let rest = if charged && h.attack < 0.001 && h.main_hand { Mat4::from_translation(Vec3::new(side * -0.641_864, 0.0, 0.0)) * rot(Vec3::Y, side * 10.0) } else { Mat4::IDENTITY };
        return arm * swing(side, h.attack) * rest;
    }
    match h.using {
        Some(Using::Eat { held, duration }) => eat(side, held, duration) * arm,
        Some(Using::Block) => arm * Mat4::from_translation(Vec3::new(side * -0.141_421_36, 0.08, 0.141_421_36)) * rot(Vec3::X, -102.25) * rot(Vec3::Y, side * 13.365) * rot(Vec3::Z, side * 78.05),
        Some(Using::Bow { held }) => {
            let turn = Mat4::from_translation(Vec3::new(side * -0.278_568_2, 0.183_443_87, 0.157_315_31)) * rot(Vec3::X, -13.935) * rot(Vec3::Y, side * 35.3) * rot(Vec3::Z, side * -9.785);
            let m = held / 20.0;
            arm * turn * drawn(side, held, ((m * m + m * 2.0) / 3.0).min(1.0), 0.04)
        }
        Some(Using::Trident { held }) => {
            let turn = Mat4::from_translation(Vec3::new(side * -0.5, 0.7, 0.1)) * rot(Vec3::X, -55.0) * rot(Vec3::Y, side * 35.3) * rot(Vec3::Z, side * -9.785);
            arm * turn * drawn(side, held, (held / 10.0).min(1.0), 0.2)
        }
        // Loading a crossbow is the crossbow branch's; another item in that use stays at rest.
        Some(Using::Crossbow { .. }) => arm,
        None if h.spinning => arm * Mat4::from_translation(Vec3::new(side * -0.4, 0.8, 0.3)) * rot(Vec3::Y, side * 65.0) * rot(Vec3::Z, side * -85.0),
        None if h.swinging => arm * swing(side, h.attack),
        None => arm,
    }
}
