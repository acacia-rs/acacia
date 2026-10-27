use super::*;

/// Flat ground at `surface`, one precipitation everywhere, light `[3, 14]`.
struct Flat {
    surface: i32,
    precipitation: Precipitation,
}

impl WeatherLevel for Flat {
    fn surface(&self, _: i32, _: i32) -> i32 {
        self.surface
    }
    fn precipitation(&self, _: [i32; 3]) -> Precipitation {
        self.precipitation
    }
    fn light(&self, _: [i32; 3]) -> [u8; 2] {
        [3, 14]
    }
}

#[test]
fn the_column_table_matches_the_constructors_arithmetic() {
    for i in 0..1024 {
        let (dx, dz) = ((i % 32) as f32 - 16.0, (i / 32) as f32 - 16.0);
        let distance = (dx * dx + dz * dz).sqrt();
        let [x, z] = COLUMN_SIZE[i];
        if i == 16 * 32 + 16 {
            assert!(x.is_nan() && z.is_nan());
        } else {
            assert_eq!((x.to_bits(), z.to_bits()), ((-dz / distance).to_bits(), (dx / distance).to_bits()), "{i}");
        }
    }
}

#[test]
fn every_column_of_the_square_rains_down_to_the_ground() {
    let level = Flat { surface: 64, precipitation: Precipitation::Rain };
    let columns = extract(&level, 1.0, 1000, 0.5, DVec3::new(0.5, 70.2, 0.5), 10);
    // The square, not a circle: 21 × 21 columns from the ground up to 10 above the eye.
    assert_eq!((columns.rain.len(), columns.snow.len()), (441, 0));
    assert!(columns.rain.iter().all(|c| c.bottom == 64 && c.top == 80 && c.light == [3, 14] && c.u_offset == 0.0));
    // Ground above the eye's reach: nothing.
    let buried = Flat { surface: 90, precipitation: Precipitation::Rain };
    assert!(extract(&buried, 1.0, 0, 0.0, DVec3::new(0.5, 70.0, 0.5), 10).rain.is_empty());
    let dry = Flat { surface: 64, precipitation: Precipitation::None };
    assert!(extract(&dry, 1.0, 0, 0.0, DVec3::new(0.5, 70.0, 0.5), 10).rain.is_empty());
    assert!(extract(&level, 0.0, 0, 0.0, DVec3::new(0.5, 70.0, 0.5), 10).rain.is_empty());
}

#[test]
fn snow_drifts_and_brightens_its_light() {
    let level = Flat { surface: 64, precipitation: Precipitation::Snow };
    let a = extract(&level, 1.0, 1000, 0.0, DVec3::new(0.5, 70.0, 0.5), 3);
    let b = extract(&level, 1.0, 1001, 0.0, DVec3::new(0.5, 70.0, 0.5), 3);
    assert_eq!(a.snow.len(), 49);
    assert!(a.snow.iter().all(|c| c.light == [6, 14]), "(l * 3 + 15) / 4");
    assert!(a.snow.iter().zip(&b.snow).all(|(a, b)| a.u_offset != b.u_offset));
}

#[test]
fn rain_falls_a_column_at_its_own_speed() {
    let level = Flat { surface: 64, precipitation: Precipitation::Rain };
    let at = |ticks| extract(&level, 1.0, ticks, 0.0, DVec3::new(0.5, 70.0, 0.5), 3).rain;
    let (a, b) = (at(100), at(101));
    let speeds: Vec<f32> = a.iter().zip(&b).map(|(a, b)| (a.v_offset - b.v_offset).rem_euclid(32.0) * 32.0).collect();
    assert!(speeds.iter().all(|s| (2.999..4.001).contains(s)), "{speeds:?}");
    assert!(speeds.windows(2).any(|w| w[0] != w[1]));
}

#[test]
fn quads_face_the_camera_and_fade_to_half_at_the_radius() {
    let columns = Columns {
        rain: vec![Column { x: 3, z: 0, bottom: 60, top: 80, u_offset: 0.0, v_offset: 0.0, light: [0, 15] }],
        snow: vec![Column { x: 0, z: 10, bottom: 60, top: 80, u_offset: 0.25, v_offset: 0.0, light: [0, 15] }],
        radius: 10,
        intensity: 1.0,
    };
    let [rain, snow] = quads(&columns, DVec3::new(0.5, 70.0, 0.5))[..] else { panic!() };
    // East of the camera: the quad spans z, a block wide; v climbs a quarter a block.
    assert_eq!(rain.map(|v| v.pos), [[3.0, 10.0, -0.5], [3.0, 10.0, 0.5], [3.0, -10.0, 0.5], [3.0, -10.0, -0.5]]);
    assert_eq!(rain.map(|v| v.uv), [[0.0, 15.0], [1.0, 15.0], [1.0, 20.0], [0.0, 20.0]]);
    assert_eq!(rain[0].alpha, ((1.0 - 0.09 * 0.5) * 255.0f32).floor() as u8);
    assert_eq!((snow[0].alpha, snow[0].uv[0]), (127, 0.25), "snow: 0.8 near, 0.5 at the radius");
}

#[test]
fn the_cameras_own_column_is_degenerate() {
    let columns = Columns { rain: vec![Column { x: 0, z: 0, bottom: 60, top: 80, u_offset: 0.0, v_offset: 0.0, light: [0, 0] }], snow: vec![], radius: 10, intensity: 1.0 };
    let quad = quads(&columns, DVec3::new(0.5, 70.0, 0.5))[0];
    assert!(quad.iter().all(|v| v.pos[0].is_nan() && v.pos[2].is_nan() && v.pos[1].is_finite()));
}
