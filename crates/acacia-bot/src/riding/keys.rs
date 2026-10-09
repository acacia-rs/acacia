//! The rider's movement keys in a seated `PlayerAuthInput` (docs/research/riding-fishing-elytra.md
//! "PlayerAuthInput while riding").

use acacia_client::proto::packets::PlayerAuthInput;
use acacia_client::proto::types::{InputData as F, Vec2f};

use crate::movement::Controls;

/// The controls' [strafe, forward] as keys: any nonzero value counts as fully pressed.
pub(super) fn keys(c: &Controls) -> [f32; 2] {
    [axis(c.strafe), axis(c.forward)]
}

fn axis(v: f32) -> f32 {
    if v > 0.0 { 1.0 } else if v < 0.0 { -1.0 } else { 0.0 }
}

/// The rider's movement keys, as vanilla reports them while riding (the pig capture: plain WASD).
pub(super) fn report_keys(input: &mut PlayerAuthInput, [strafe, forward]: [f32; 2]) {
    let held = [(forward > 0.0, F::Up), (forward < 0.0, F::Down), (strafe > 0.0, F::Left), (strafe < 0.0, F::Right)];
    push_held(input, held);
    input.move_vector = Vec2f { x: strafe, z: forward };
    input.raw_move_vector = Vec2f { x: strafe, z: forward };
}

/// A boat driver's paddles: the left one with Left, the right one with Right (the boat capture).
pub(super) fn report_paddles(input: &mut PlayerAuthInput, strafe: f32) {
    push_held(input, [(strafe > 0.0, F::PaddlingLeft), (strafe < 0.0, F::PaddlingRight)]);
}

fn push_held<const N: usize>(input: &mut PlayerAuthInput, held: [(bool, F); N]) {
    for (on, flag) in held {
        if on && !input.input_data.contains(&flag) {
            input.input_data.push(flag);
        }
    }
}
