//! `HotSnapshot` as numbers, so a per-tick `bot.hot` crosses no JSON (packages/acacia-bot/src/native-hot.ts reads
//! them back in this order).

use acacia_api::HotSnapshot;

/// Numbers in a hot array: tick, position xyz, velocity xyz, yaw, pitch, onGround, health, maxHealth, food,
/// saturation, xpLevel, xpProgress, air, alive, gameMode, dimension, heldSlot, openWindow (NaN for none), flying,
/// sleeping, riding, gliding. Booleans are 0 or 1.
pub const HOT_NUMBERS: usize = 26;

pub fn numbers(h: &HotSnapshot) -> [f64; HOT_NUMBERS] {
    let b = |v: bool| f64::from(u8::from(v));
    let (p, v) = (h.position, h.velocity);
    [
        h.tick as f64,
        p.x.into(),
        p.y.into(),
        p.z.into(),
        v.x.into(),
        v.y.into(),
        v.z.into(),
        h.yaw.into(),
        h.pitch.into(),
        b(h.on_ground),
        h.health.into(),
        h.max_health.into(),
        h.food.into(),
        h.saturation.into(),
        h.xp_level.into(),
        h.xp_progress.into(),
        h.air.into(),
        b(h.alive),
        h.game_mode.into(),
        h.dimension.into(),
        h.held_slot.into(),
        h.open_window.map_or(f64::NAN, f64::from),
        b(h.flying),
        b(h.sleeping),
        b(h.riding),
        b(h.gliding),
    ]
}
