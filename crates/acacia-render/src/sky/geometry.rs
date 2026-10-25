//! `SkyRenderer`'s geometry, camera-relative in blocks (x east, y up, z south): the sky and dark
//! discs, the sunrise fan, the sun, moon and stars on their turning frames, and the End's box.

use std::f32::consts::FRAC_PI_2;

use glam::{Mat4, Vec3};

use crate::java_mth as mth;
use crate::java_random::JavaRandom;

/// Degrees as `SkyRenderer` turns them: times the float `PI / 180`.
pub fn radians(degrees: f32) -> f32 {
    degrees * (std::f64::consts::PI / 180.0) as f32
}

/// A quad's corners in drawing order, each with its texture coordinate.
pub type Quad = [([f32; 3], [f32; 2]); 4];

pub const SUN_SIZE: f32 = 30.0;
pub const MOON_SIZE: f32 = 20.0;
/// Height of the sky disc, and of the dark disc's model ([`dark_disc_frame`]).
pub const SKY_DISC_Y: f32 = 16.0;
pub const DARK_DISC_Y: f32 = -16.0;

/// `buildSkyDisc`: a fan from overhead to eight rim points 512 blocks out (`i` from -180 to 180
/// by 45 degrees), mirrored below the horizon.
pub fn sky_disc(y: f32) -> [[f32; 3]; 10] {
    let radius = 512.0f32.copysign(y);
    let mut out = [[0.0, y, 0.0]; 10];
    for (k, slot) in out[1..].iter_mut().enumerate() {
        let a = f64::from(((k as i32 - 4) * 45) as f32 * (std::f64::consts::PI / 180.0) as f32);
        *slot = [radius * mth::cos(a), y, 512.0 * mth::sin(a)];
    }
    out
}

/// `SkyRenderer.shouldRenderDarkDisc`: the eye under the horizon (`getHorizonHeight`: 63, a flat
/// world's floor) and not under water.
pub fn dark_disc_shown(camera_y: f64, horizon: f64, underwater: bool) -> bool {
    camera_y < horizon && !underwater
}

/// The frame a body turns on: `rotateY(-90°)` then `rotateX(angle)`.
pub fn celestial_frame(angle_degrees: f32) -> Mat4 {
    Mat4::from_rotation_y((-90.0f32).to_radians()) * Mat4::from_rotation_x(angle_degrees.to_radians())
}

/// The sun's quad 100 blocks out on its frame, `size` its half-width, its texture whole.
pub fn body(frame: Mat4, size: f32) -> Quad {
    body_with_uvs(frame, size, [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]])
}

/// `buildMoonPhases`: the moon's quad takes its sprite turned half round.
pub fn moon(frame: Mat4) -> Quad {
    body_with_uvs(frame, MOON_SIZE, [[1.0, 1.0], [0.0, 1.0], [0.0, 0.0], [1.0, 0.0]])
}

fn body_with_uvs(frame: Mat4, size: f32, uvs: [[f32; 2]; 4]) -> Quad {
    let corner = |x: f32, z: f32, uv: [f32; 2]| (frame.transform_point3(Vec3::new(x, 100.0, z)).to_array(), uv);
    [corner(-size, -size, uvs[0]), corner(size, -size, uvs[1]), corner(size, size, uvs[2]), corner(-size, size, uvs[3])]
}

/// `SkyRenderer.buildStars`: up to 1500 quads from seed 10842, 100 blocks out, each turned to face
/// the centre and spun about its own axis; before the star frame.
pub fn stars() -> Vec<[[f32; 3]; 4]> {
    let mut random = JavaRandom::new(10842);
    let mut out = Vec::with_capacity(1500);
    for _ in 0..1500 {
        let (x, y, z) = (random.next_float() * 2.0 - 1.0, random.next_float() * 2.0 - 1.0, random.next_float() * 2.0 - 1.0);
        let size = 0.15 + random.next_float() * 0.1;
        let l = x * x + y * y + z * z;
        if l <= 0.010_000_001 || l >= 1.0 {
            continue;
        }
        let center = Vec3::new(x, y, z).normalize() * 100.0;
        let spin = (random.next_double() * f64::from(std::f32::consts::PI) * 2.0) as f32;
        // Matrix3f.rotateTowards(-center, up).rotateZ(-spin).
        let forward = -center.normalize();
        let right = Vec3::Y.cross(forward).normalize_or(Vec3::X);
        let up = forward.cross(right);
        let (s, c) = (-spin).sin_cos();
        let (r, u) = (right * c + up * s, up * c - right * s);
        let corner = |a: f32, b: f32| (center + r * a + u * b).to_array();
        out.push([corner(size, -size), corner(size, size), corner(-size, size), corner(-size, -size)]);
    }
    out
}

/// `buildSunriseFan`: the centre, then its 17 rim points (the first repeated last). The centre
/// takes the sunrise colour, the rim the same at alpha 0.
pub fn sunrise_fan_model() -> [[f32; 3]; 18] {
    let mut out = [[0.0, 100.0, 0.0]; 18];
    for (i, slot) in out[1..].iter_mut().enumerate() {
        let a = f64::from(i as f32 * std::f32::consts::TAU / 16.0);
        let (sin, cos) = (mth::sin(a), mth::cos(a));
        *slot = [sin * 120.0, cos * 120.0, -cos * 40.0];
    }
    out
}

/// `renderSunriseAndSunset`'s pose: rotateX(90°), rotateZ(180° once the sun is past its zenith,
/// plus 90°), scaled by the colour's alpha along z.
pub fn sunrise_frame(sun_angle_degrees: f32, alpha: f32) -> Mat4 {
    let flip = if mth::sin(f64::from(radians(sun_angle_degrees))) < 0.0 { 180.0f32 } else { 0.0 };
    Mat4::from_rotation_x(FRAC_PI_2) * Mat4::from_rotation_z((flip + 90.0).to_radians()) * Mat4::from_scale(Vec3::new(1.0, 1.0, alpha))
}

/// The fan as drawn: [`sunrise_fan_model`] through [`sunrise_frame`].
pub fn sunrise_fan(sun_angle_degrees: f32, alpha: f32) -> [[f32; 3]; 18] {
    let frame = sunrise_frame(sun_angle_degrees, alpha);
    sunrise_fan_model().map(|p| frame.transform_point3(Vec3::from_array(p)).to_array())
}

/// The dark disc's pose: `buildSkyDisc(-16)` drawn 12 blocks up. Its fog still measures from
/// the unmoved disc, as Java's `sky.vsh` reads the model position.
pub fn dark_disc_frame() -> Mat4 {
    Mat4::from_translation(Vec3::new(0.0, 12.0, 0.0))
}

/// `sky.fsh`'s fog at a disc's model point: spherical over 0..`sky_end`, and whole wherever the
/// cylindrical distance reaches `sky_end`.
pub fn sky_disc_fog(model: [f32; 3], sky_end: f32) -> f32 {
    let p = Vec3::from_array(model);
    let cylindrical = p.x.hypot(p.z).max(p.y.abs());
    let spherical = (p.length() / sky_end).clamp(0.0, 1.0);
    if cylindrical >= sky_end { 1.0 } else { spherical }
}

/// `buildEndSky`: six faces 100 blocks out, each `end_sky.png` tiled 16 times (texture
/// coordinates run 0..16; the sampler repeats them).
pub fn end_sky() -> [Quad; 6] {
    use std::f32::consts::PI;
    let poses = [
        Mat4::IDENTITY,
        Mat4::from_rotation_x(FRAC_PI_2),
        Mat4::from_rotation_x(-FRAC_PI_2),
        Mat4::from_rotation_x(PI),
        Mat4::from_rotation_z(FRAC_PI_2),
        Mat4::from_rotation_z(-FRAC_PI_2),
    ];
    poses.map(|pose| {
        let corner = |x: f32, z: f32, uv: [f32; 2]| (pose.transform_point3(Vec3::new(x, -100.0, z)).to_array(), uv);
        [corner(-100.0, -100.0, [0.0, 0.0]), corner(-100.0, 100.0, [0.0, 16.0]), corner(100.0, 100.0, [16.0, 16.0]), corner(100.0, -100.0, [16.0, 0.0])]
    })
}

/// The End sky's tint (`0x282828`).
pub const END_SKY_SHADE: f32 = 40.0 / 255.0;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stars_lie_on_the_sphere_and_stay_the_same() {
        let stars = stars();
        assert!((700..1500).contains(&stars.len()), "{}", stars.len());
        for p in stars.iter().flatten() {
            assert!((99.9..100.1).contains(&Vec3::from_array(*p).length()));
        }
        assert_eq!(stars, super::stars());
    }

    #[test]
    fn the_sun_is_overhead_at_noon_and_sets_in_the_west() {
        let centre = |angle: f32| body(celestial_frame(angle), SUN_SIZE).iter().fold(Vec3::ZERO, |s, c| s + Vec3::from_array(c.0)) / 4.0;
        assert!((centre(0.0) - Vec3::new(0.0, 100.0, 0.0)).length() < 1e-3);
        assert!((centre(90.0) - Vec3::new(-100.0, 0.0, 0.0)).length() < 1e-3, "{}", centre(90.0));
        // The fan sits under the setting sun, its centre on the west horizon.
        let fan = sunrise_fan(90.0, 1.0);
        assert!((Vec3::from_array(fan[0]) - Vec3::new(-100.0, 0.0, 0.0)).length() < 1e-3, "{:?}", fan[0]);
        assert!(Vec3::from_array(sunrise_fan(270.0, 1.0)[0]).x > 99.0, "sunrise in the east");
        assert_eq!(fan[1], fan[17]);
    }

    #[test]
    fn discs_and_the_end_box() {
        let disc = sky_disc(SKY_DISC_Y);
        assert_eq!(disc[0], [0.0, 16.0, 0.0]);
        assert!((disc[1][0] + 512.0).abs() < 1e-3 && disc[1][2].abs() < 1e-3);
        assert!(disc[1..].iter().all(|p| (p[0].hypot(p[2]) - 512.0).abs() < 0.03) && disc[1][0] == disc[9][0]);
        // Mth.cos(-45°) truncates its table index towards zero: one rim point sits a little in.
        assert_eq!(disc[4][0], 362.003_97);
        assert!((sky_disc(-16.0)[1][0] - 512.0).abs() < 1e-3, "mirrored below");
        assert!(dark_disc_shown(62.9, 63.0, false) && !dark_disc_shown(63.0, 63.0, false) && !dark_disc_shown(10.0, 63.0, true));
        assert_eq!((sky_disc_fog([0.0, 16.0, 0.0], 192.0), sky_disc_fog([192.0, 16.0, 0.0], 192.0)), (16.0 / 192.0, 1.0));
        let (sun, moon) = (body(celestial_frame(0.0), MOON_SIZE), moon(celestial_frame(0.0)));
        assert!(sun.iter().zip(moon).all(|(s, m)| s.0 == m.0 && s.1 == [1.0 - m.1[0], 1.0 - m.1[1]]));
        for face in end_sky() {
            for (p, _) in face {
                assert!(p.iter().any(|c| (c.abs() - 100.0).abs() < 1e-3));
            }
        }
    }
}
