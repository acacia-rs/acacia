//! 26.3's entity colour overlay and shaking: `OverlayTexture` (a red row for the hurt and dying, a
//! white ramp for a swelling creeper), `LivingEntityRenderer.getOverlayCoords`, and the shake of
//! the frozen and the converting (`isShaking`, `setupRotations`). Checked by the parity harness
//! `overlays`.

/// `OverlayTexture.u`: the white ramp's column.
pub fn u(white: f32) -> i32 {
    (white * 15.0) as i32
}

/// The overlay texel at (`u`, row 3 when `hurt`, else 10), ARGB.
pub fn texel(u: i32, hurt: bool) -> u32 {
    if hurt {
        return 0xB2FF_0000;
    }
    let a = ((1.0 - u as f32 / 15.0 * 0.75) * 255.0) as u32;
    a << 24 | 0x00FF_FFFF
}

/// What the entity shader takes: how much of the model's own colour stays (the texel's alpha),
/// negative for the red overlay (white otherwise). 1 is no overlay.
pub fn keep(hurt: bool, white: f32) -> f32 {
    let alpha = (texel(u(white), hurt) >> 24) as f32 / 255.0;
    if hurt { -alpha } else { alpha }
}

/// `CreeperRenderer.getWhiteOverlayProgress`: flashing while it swells.
pub fn creeper_white(swelling: f32) -> f32 {
    if (swelling * 10.0) as i32 % 2 == 0 { 0.0 } else { swelling.clamp(0.5, 1.0) }
}

/// Why an entity might shake: fully frozen in powder snow, or converting (a zombie drowning, a
/// piglin or hoglin zombifying, a zombie villager curing, a skeleton freezing into a stray), or a
/// strider out of lava.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Shake {
    pub frozen: bool,
    pub converting: bool,
    pub suffocating: bool,
}

/// The renderer's `isShaking` for a Java `kind`.
pub fn shaking(kind: &str, s: Shake) -> bool {
    match kind.trim_start_matches("minecraft:") {
        // AbstractSkeletonRenderer: only its own conversion, not the frost.
        "skeleton" | "stray" | "wither_skeleton" | "bogged" | "parched" => s.converting,
        "zombie" | "husk" | "drowned" | "zombie_villager" | "piglin" | "hoglin" => s.frozen || s.converting,
        "strider" => s.frozen || s.suffocating,
        _ => s.frozen,
    }
}

/// Degrees `setupRotations` adds to the body's yaw of a shaking entity `age` ticks old (π × 0.4
/// at most: Java adds radians-looking numbers to degrees).
pub fn shake_degrees(age: f32) -> f32 {
    (f64::from(age.floor() as i32 as f32 * 3.25).cos() * std::f64::consts::PI * f64::from(0.4f32)) as f32
}
