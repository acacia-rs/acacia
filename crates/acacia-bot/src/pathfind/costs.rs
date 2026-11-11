//! Move costs in ticks (Baritone's `ActionCosts`, as ported by azalea): the movement-derived ones from the
//! edition's [`Envelope`], the penalties edition-free. Bedrock shares Java's walk/sprint speeds and gravity, so
//! the tables carry over unchanged.

use super::profile::MovementProfile;
use crate::movement::Envelope;

pub fn walk(e: &Envelope) -> f32 {
    20.0 / e.walk_speed
}

pub fn sprint(e: &Envelope) -> f32 {
    20.0 / e.sprint_speed
}

pub fn walk_off(e: &Envelope) -> f32 {
    walk(e) * 0.8
}

pub fn center_after_fall(e: &Envelope) -> f32 {
    walk(e) - walk_off(e)
}

pub fn swim(e: &Envelope) -> f32 {
    20.0 / e.swim_speed
}

pub fn climb(e: &Envelope) -> f32 {
    1.0 / e.climb_speed
}

/// Sneaking to an edge to bridge (Baritone `SNEAK_ONE_BLOCK_COST`).
pub fn sneak(e: &Envelope) -> f32 {
    20.0 / e.sneak_speed
}

pub const JUMP_PENALTY: f32 = 2.0;
pub const ENTER_WATER: f32 = 3.0;
/// Standing next to cactus, fire or lava: allowed, but only when the detour is long.
pub const DANGER_NEAR: f32 = 20.0;
/// Extra risk for a two-block gap jump over a one-block one.
pub const LONG_JUMP_PENALTY: f32 = 2.0;
/// Least ticks a run from standing loses against steady hops over three blocks or more (measured 4.1 at the
/// first landing; `tests::hops::steady_hop_speed_bounds_the_heuristic` checks the bound).
pub const MACRO_STARTUP: f32 = 4.0;
/// Ticks a classic move from standing loses getting up to speed, which its per-block cost leaves out (measured
/// 4.2–4.7 over 5–60 blocks by `tests::perf::cost_skew_report`; the other ~3 of the skew is settling at the goal,
/// the same for every plan). Macro moves simulate theirs.
pub const CLASSIC_STARTUP: f32 = 4.0;

/// Fewest ticks per block any move achieves with macro moves on: steady hops (plus the `Human` jump
/// hesitation, unless plain sprinting is faster), rounded down so the heuristic stays admissible.
pub fn macro_ticks_per_block(e: &Envelope, profile: MovementProfile) -> f32 {
    let hop = match profile {
        MovementProfile::Optimal => 1.0 / e.hop_speed,
        MovementProfile::Human => ((e.hop_ticks + HUMAN_JUMP) / (e.hop_ticks * e.hop_speed)).min(sprint(e)),
    };
    (hop * 100.0).floor() / 100.0
}
/// On top of the break time, for tool switches and the risk (Baritone `blockBreakAdditionalPenalty`).
pub const BREAK_PENALTY: f32 = 2.0;
/// Blocks that take longer than this to break (30 s) are not dug.
pub const MAX_DIG_TICKS: u32 = 600;
/// Placing a scaffold block (Baritone `placeBlockCost`).
pub const PLACE: f32 = 20.0;
/// Opening a door or gate: aim, click, wait for the update.
pub const DOOR: f32 = 10.0;
/// `Human`: a person hesitates before each jump of a macro move.
pub const HUMAN_JUMP: f32 = 2.0;
/// `Human`: cost of a landing with no margin, falling to 0 at [`RISK_MARGIN`] blocks from the cell's sides.
pub const RISK: f32 = 6.0;
pub const RISK_MARGIN: f32 = 0.3;

/// Placing a block with `left` scaffold blocks remaining: dearer as they run out.
pub fn place(left: u32) -> f32 {
    PLACE * (1.0 + 4.0 / left.max(1) as f32)
}

/// Ticks to fall `distance` blocks from rest (the first tick moves before gravity applies).
pub fn fall_ticks(e: &Envelope, distance: f32) -> f32 {
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
        v = (v + e.gravity) * e.air_drag;
    }
}

pub fn jump_one_block(e: &Envelope) -> f32 {
    fall_ticks(e, e.jump_rise) - fall_ticks(e, e.jump_rise - 1.0)
}

pub fn ascend(e: &Envelope) -> f32 {
    jump_one_block(e).max(walk(e)) + JUMP_PENALTY
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::movement::BEDROCK;

    #[test]
    fn fall_table_matches_baritone() {
        let e = &BEDROCK;
        assert!((fall_ticks(e, 1.0) - 5.61).abs() < 0.05, "{}", fall_ticks(e, 1.0));
        assert!((fall_ticks(e, 3.0) - 9.47).abs() < 0.05, "{}", fall_ticks(e, 3.0));
        assert!((jump_one_block(e) - 3.163).abs() < 0.05);
    }

    #[test]
    fn scarce_blocks_cost_more() {
        assert!(place(1) > place(8) && place(8) > place(64));
        assert!((place(64) - PLACE).abs() < 2.0);
    }
}
