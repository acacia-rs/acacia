//! Move costs in ticks (Baritone's `ActionCosts`, as ported by azalea). Bedrock shares Java's
//! walk/sprint speeds and gravity, so the tables carry over unchanged.

use acacia_physics::constants::{CLIMB_SPEED, NORMAL_GRAVITY, NORMAL_GRAVITY_MULTIPLIER};

pub const WALK: f32 = 20.0 / 4.317;
pub const SPRINT: f32 = 20.0 / 5.612;
pub const WALK_OFF: f32 = WALK * 0.8;
pub const CENTER_AFTER_FALL: f32 = WALK - WALK_OFF;
pub const JUMP_PENALTY: f32 = 2.0;
pub const SWIM: f32 = 20.0 / 1.960;
pub const ENTER_WATER: f32 = 3.0;
pub const CLIMB: f32 = 1.0 / CLIMB_SPEED;
/// Standing next to cactus, fire or lava: allowed, but only when the detour is long.
pub const DANGER_NEAR: f32 = 20.0;
/// Extra risk for a two-block gap jump over a one-block one.
pub const LONG_JUMP_PENALTY: f32 = 2.0;
/// Sprint cost per block: turns heuristic distance into ticks (Baritone `costHeuristic`).
pub const COST_HEURISTIC: f32 = 3.563;

/// Ticks to fall `distance` blocks from rest (the first tick moves before gravity applies).
pub fn fall_ticks(distance: f32) -> f32 {
    if distance <= 0.0 {
        return 0.0;
    }
    let (mut v, mut left, mut ticks) = (0.0f32, distance, 0.0f32);
    loop {
        if v >= left {
            return ticks + left / v;
        }
        left -= v;
        ticks += 1.0;
        v = (v + NORMAL_GRAVITY) * NORMAL_GRAVITY_MULTIPLIER;
    }
}

pub fn jump_one_block() -> f32 {
    fall_ticks(1.25) - fall_ticks(0.25)
}

pub fn ascend() -> f32 {
    jump_one_block().max(WALK) + JUMP_PENALTY
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fall_table_matches_baritone() {
        assert!((fall_ticks(1.0) - 5.61).abs() < 0.05, "{}", fall_ticks(1.0));
        assert!((fall_ticks(3.0) - 9.47).abs() < 0.05, "{}", fall_ticks(3.0));
        assert!((jump_one_block() - 3.163).abs() < 0.05);
    }
}
