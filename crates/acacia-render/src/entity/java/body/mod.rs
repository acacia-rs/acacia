//! The whole-body transform `LivingEntityRenderer.submit` gives a model before it draws (26.3
//! client): the entity's scale, `setupRotations` (the body's turn, death, spin attacks, sleep,
//! Dinnerbone, and each renderer's own: fish flop, swimming, rolling pandas, attached shulkers...),
//! the `scale(-1, -1, 1)` flip, the renderer's `scale` and the 1.501 drop. In Java's frame,
//! without the entity's position; the harness `body_transforms` proves it.

mod panda;

use std::f32::consts::PI;

use glam::{Mat4, Vec3};

use super::mth::{cos, joml_rotation, lerp, rot_lerp, sin};
use super::overlay::{self, Shake};
use super::state::RenderState;

/// A PoseStack's pose matrix.
struct Stack(Mat4);

impl Stack {
    fn turn(&mut self, axis: Vec3, degrees: f32) {
        self.0 *= joml_rotation(axis, degrees);
    }
    fn shift(&mut self, x: f32, y: f32, z: f32) {
        self.0 *= Mat4::from_translation(Vec3::new(x, y, z));
    }
    fn scale(&mut self, x: f32, y: f32, z: f32) {
        self.0 *= Mat4::from_scale(Vec3::new(x, y, z));
    }
    /// `rotateAround`: about a pivot.
    fn turn_about(&mut self, rotation: Mat4, pivot: Vec3) {
        self.0 *= Mat4::from_translation(pivot) * rotation * Mat4::from_translation(-pivot);
    }
}

/// `Direction` by its Java name.
fn direction(s: &RenderState, field: &str) -> Option<&'static str> {
    let name = s.text(field)?.to_ascii_uppercase();
    ["DOWN", "UP", "NORTH", "SOUTH", "WEST", "EAST"].into_iter().find(|d| *d == name)
}

/// `Direction.getRotation`.
fn face_rotation(face: &str) -> Mat4 {
    let (half, pi) = (PI / 2.0, PI);
    let xyz = |x: f32, z: f32| Mat4::from_rotation_x(x) * Mat4::from_rotation_z(z);
    match face {
        "DOWN" => Mat4::from_rotation_x(pi),
        "NORTH" => xyz(half, pi),
        "SOUTH" => Mat4::from_rotation_x(half),
        "WEST" => xyz(half, half),
        "EAST" => xyz(half, -half),
        _ => Mat4::IDENTITY,
    }
}

fn opposite(face: &str) -> &'static str {
    match face {
        "DOWN" => "UP",
        "UP" => "DOWN",
        "NORTH" => "SOUTH",
        "SOUTH" => "NORTH",
        "WEST" => "EAST",
        _ => "WEST",
    }
}

/// `getFlipDegrees`: arthropods fall on their backs.
fn flip_degrees(kind: &str) -> f32 {
    if matches!(kind, "spider" | "cave_spider" | "silverfish" | "endermite") { 180.0 } else { 90.0 }
}

/// The matrix for a Java `kind` (`minecraft:cod`) in render state `s`.
pub fn transform(kind: &str, s: &RenderState) -> Mat4 {
    let kind = kind.trim_start_matches("minecraft:");
    let mut m = Stack(Mat4::IDENTITY);
    let sleeping = s.text("pose") == Some("SLEEPING");
    if sleeping && let Some(bed) = direction(s, "bedOrientation") {
        let head = s.num("eyeHeight") - 0.1;
        let (x, z) = match bed {
            "NORTH" => (0.0, -1.0),
            "SOUTH" => (0.0, 1.0),
            "WEST" => (-1.0, 0.0),
            "EAST" => (1.0, 0.0),
            _ => (0.0, 0.0),
        };
        m.shift(-x * head, 0.0, -z * head);
    }
    let scale = if s.has("scale") { s.num("scale") } else { 1.0 };
    m.scale(scale, scale, scale);
    setup_rotations(kind, s, &mut m, s.num("bodyRot"), scale, sleeping);
    m.scale(-1.0, -1.0, 1.0);
    renderer_scale(kind, s, &mut m);
    m.shift(0.0, -1.501, 0.0);
    m.0
}

/// `LivingEntityRenderer.setupRotations` and the renderers' overrides.
fn setup_rotations(kind: &str, s: &RenderState, m: &mut Stack, body_rot: f32, scale: f32, sleeping: bool) {
    match kind {
        "armor_stand" => {
            m.turn(Vec3::Y, 180.0 - body_rot);
            let wiggle = s.num("wiggle");
            if wiggle < 5.0 {
                m.turn(Vec3::Y, sin(wiggle / 1.5 * PI) * 3.0);
            }
            return;
        }
        "squid" | "glow_squid" => {
            let baby = s.baby();
            m.shift(0.0, if baby { 0.25 } else { 0.5 }, 0.0);
            m.turn(Vec3::Y, 180.0 - body_rot);
            m.turn(Vec3::X, s.num("xBodyRot"));
            m.turn(Vec3::Y, s.num("zBodyRot"));
            m.shift(0.0, if baby { -0.6 } else { -1.2 }, 0.0);
            return;
        }
        "pufferfish" => m.shift(0.0, cos(s.age() * 0.05) * 0.08, 0.0),
        "shulker" => {
            living_rotations(kind, s, m, body_rot + 180.0, scale, sleeping);
            let face = direction(s, "attachFace").unwrap_or("DOWN");
            m.turn_about(face_rotation(opposite(face)), Vec3::new(0.0, 0.5, 0.0));
            return;
        }
        "player" | "mannequin" => return avatar_rotations(kind, s, m, body_rot, scale, sleeping),
        _ => {}
    }
    living_rotations(kind, s, m, body_rot, scale, sleeping);
    let in_water = s.flag("isInWater");
    match kind {
        "cat" => {
            let lie = s.num("lieDownAmount");
            if lie > 0.0 {
                m.shift(0.4 * lie, 0.15 * lie, 0.1 * lie);
                m.turn(Vec3::Z, rot_lerp(lie, 0.0, 90.0));
                if s.flag("isLyingOnTopOfSleepingPlayer") {
                    m.shift(0.15 * lie, 0.0, 0.0);
                }
            }
        }
        "cod" | "tropical_fish" | "salmon" => {
            let (amplitude, angle) = if kind == "salmon" && !in_water { (1.3, 1.7) } else { (1.0, 1.0) };
            m.turn(Vec3::Y, amplitude * 4.3 * sin(angle * 0.6 * s.age()));
            if !in_water {
                if kind == "cod" { m.shift(0.1, 0.1, -0.1) } else { m.shift(0.2, 0.1, 0.0) }
                m.turn(Vec3::Z, 90.0);
            }
        }
        "drowned" => {
            let swim = s.num("swimAmount");
            if swim > 0.0 {
                let angle = lerp(swim, 0.0, -10.0 - s.x_rot());
                let pivot = Vec3::new(0.0, s.num("boundingBoxHeight") / 2.0 / scale, 0.0);
                m.turn_about(Mat4::from_quat(glam::Quat::from_rotation_x(angle.to_radians())), pivot);
            }
        }
        "fox" if s.flag("isPouncing") || s.flag("isFaceplanted") => m.turn(Vec3::X, -s.x_rot()),
        "iron_golem" if f64::from(s.walk_speed()) >= 0.01 => {
            let wave = ((s.walk_pos() + 6.0) % 13.0 - 6.5).abs() - 3.25;
            m.turn(Vec3::Z, 6.5 * (wave / 3.25));
        }
        "panda" => panda::rotations(s, m),
        "phantom" => m.turn(Vec3::X, s.x_rot()),
        _ => {}
    }
}

/// `LivingEntityRenderer.setupRotations` itself.
fn living_rotations(kind: &str, s: &RenderState, m: &mut Stack, mut body_rot: f32, scale: f32, sleeping: bool) {
    let shake = Shake { frozen: s.flag("isFullyFrozen"), converting: s.flag("isConverting") || s.flag("isShaking"), suffocating: s.flag("isSuffocating") };
    if overlay::shaking(kind, shake) {
        body_rot += overlay::shake_degrees(s.age());
    }
    if !sleeping {
        m.turn(Vec3::Y, 180.0 - body_rot);
    }
    let death = s.num("deathTime");
    if death > 0.0 {
        let fall = ((death - 1.0) / 20.0 * 1.6).sqrt().min(1.0);
        m.turn(Vec3::Z, fall * flip_degrees(kind));
    } else if s.flag("isAutoSpinAttack") {
        m.turn(Vec3::X, -90.0 - s.x_rot());
        m.turn(Vec3::Y, s.age() * -75.0);
    } else if sleeping {
        let angle = match direction(s, "bedOrientation") {
            Some("SOUTH") => 90.0,
            Some("NORTH") => 270.0,
            Some("EAST") => 180.0,
            Some(_) => 0.0,
            None => body_rot,
        };
        m.turn(Vec3::Y, angle);
        m.turn(Vec3::Z, flip_degrees(kind));
        m.turn(Vec3::Y, 270.0);
    } else if s.flag("isUpsideDown") {
        m.shift(0.0, (s.num("boundingBoxHeight") + 0.1) / scale, 0.0);
        m.turn(Vec3::Z, 180.0);
    }
}

/// `AvatarRenderer.setupRotations`: gliding and swimming tip the body over.
fn avatar_rotations(kind: &str, s: &RenderState, m: &mut Stack, body_rot: f32, scale: f32, sleeping: bool) {
    living_rotations(kind, s, m, body_rot, scale, sleeping);
    let (swim, x_rot) = (s.num("swimAmount"), s.x_rot());
    if s.flag("isFallFlying") {
        let time = s.num("fallFlyingTimeInTicks");
        let flying = (time * time / 100.0).clamp(0.0, 1.0);
        if !s.flag("isAutoSpinAttack") {
            m.turn(Vec3::X, flying * (-90.0 - x_rot));
        }
        if s.flag("shouldApplyFlyingYRot") {
            m.0 *= Mat4::from_quat(glam::Quat::from_rotation_y(s.num("flyingYRot")));
        }
    } else if swim > 0.0 {
        let target = if s.flag("isInWater") { -90.0 - x_rot } else { -90.0 };
        m.turn(Vec3::X, lerp(swim, 0.0, target));
        if s.flag("isVisuallySwimming") {
            m.shift(0.0, -1.0, 0.3);
        }
    }
}

/// The renderers' `scale`.
fn renderer_scale(kind: &str, s: &RenderState, m: &mut Stack) {
    match kind {
        "player" | "mannequin" => m.scale(0.9375, 0.9375, 0.9375),
        "creeper" => {
            let mut g = s.num("swelling");
            let wobble = 1.0 + sin(g * 100.0) * g * 0.01;
            g = g.clamp(0.0, 1.0);
            g *= g;
            g *= g;
            m.scale((1.0 + g * 0.4) * wobble, (1.0 + g * 0.1) / wobble, (1.0 + g * 0.4) * wobble);
        }
        "phantom" => {
            let scale = 1.0 + 0.15 * s.num("size");
            m.scale(scale, scale, scale);
            m.shift(0.0, 1.3125, 0.1875);
        }
        "wither" => {
            let ticks = s.num("invulnerableTicks");
            let scale = if ticks > 0.0 { 2.0 - ticks / 220.0 * 0.5 } else { 2.0 };
            m.scale(scale, scale, scale);
        }
        "slime" | "magma_cube" | "sulfur_cube" => {
            if kind != "magma_cube" {
                m.scale(0.999, 0.999, 0.999);
                m.shift(0.0, 0.001, 0.0);
            }
            let size = s.num("size");
            let squish = s.num("squish") / (size * 0.5 + 1.0);
            let w = 1.0 / (squish + 1.0);
            m.scale(w * size, 1.0 / w * size, w * size);
            if kind == "sulfur_cube" {
                sulfur_cube(s, m);
            }
        }
        _ => {}
    }
}

/// `SulfurCubeRenderer.scale` after the cube's size and squish.
fn sulfur_cube(s: &RenderState, m: &mut Stack) {
    let fuse = s.num("fuseRemainingTicks");
    if fuse < 10.0 && fuse > 0.0 {
        let mut g = (1.0 - fuse / 10.0).clamp(0.0, 1.0);
        g *= g;
        g *= g;
        let swell = 1.0 + g * 0.3;
        m.scale(swell, swell, swell);
    }
    let baby = s.baby();
    let down = if baby { 1.0 } else { 0.5 };
    let up = if s.flag("isInvisible") { 0.0 } else { 1.0 } / 16.0;
    m.scale(down, down, down);
    m.shift(-0.0, if baby { 1.24 } else { 0.98 } - up, -0.0);
}
