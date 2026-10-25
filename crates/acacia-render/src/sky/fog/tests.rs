use super::*;

fn inputs() -> FogInputs {
    FogInputs {
        fog_type: FogType::Atmospheric,
        render_distance_chunks: 12,
        cloud_range_chunks: 128,
        camera_y: 70.0,
        min_y: -64,
        void_onset: 32.0,
        forward: [0.0, 0.0, 1.0],
        rain: 0.0,
        thunder: 0.0,
        player: PlayerFog::default(),
        water_vision: 0.0,
        rain_fog: 0.0,
    }
}

fn grey(v: f32) -> Attributes {
    let mut a = Attributes::default();
    a.set(Attr::FogColor, [v, v, v, 1.0]);
    a.set(Attr::SkyColor, [v, v, v, 1.0]);
    a
}

#[test]
fn boss_world_fog_closes_in_and_water_takes_over() {
    let mut input = inputs();
    input.player.boss_world_fog = true;
    let fog = setup_fog(&Attributes::default(), &input);
    assert_eq!((fog.environmental_start, fog.environmental_end, fog.sky_end, fog.cloud_end), (0.0, 96.0, 96.0, 96.0));
    input.fog_type = FogType::Water;
    assert_eq!(setup_fog(&Attributes::default(), &input).environmental_start, -8.0);
}

#[test]
fn void_darkness_starts_32_above_the_floor() {
    let mut input = inputs();
    (input.min_y, input.camera_y) = (0, 16.0);
    assert_eq!(setup_fog(&grey(0.5), &input).color[0], 0.5 * 0.25);
    input.camera_y = 32.0;
    assert_eq!(setup_fog(&grey(0.5), &input).color[0], 0.5);
    // Lava keeps its colour in the void.
    (input.camera_y, input.fog_type) = (-10.0, FogType::Lava);
    assert_eq!(setup_fog(&grey(0.5), &input).color, LAVA);
}

#[test]
fn render_distance_band() {
    let fog = setup_fog(&grey(0.5), &inputs());
    // 12 chunks = 192 blocks, the band clamp(19.2, 4, 64).
    assert!((fog.render_distance_start - (192.0 - 19.2)).abs() < 1e-4);
    assert_eq!((fog.render_distance_end, fog.sky_end), (192.0, 192.0));
}

#[test]
fn cloud_fog_ends_at_the_cloud_distance() {
    let mut input = inputs();
    (input.render_distance_chunks, input.cloud_range_chunks) = (2, 64);
    assert_eq!(setup_fog(&Attributes::default(), &input).cloud_end, 1024.0);
    input.cloud_range_chunks = 128;
    assert_eq!(setup_fog(&Attributes::default(), &input).cloud_end, 2048.0);
}

#[test]
fn rain_fog_pulls_the_haze_in() {
    let input = FogInputs { rain_fog: 1.0, ..inputs() };
    let fog = setup_fog(&Attributes::default(), &input);
    assert_eq!((fog.environmental_start, fog.environmental_end), (-160.0, 768.0));
    let mut state = RainFog::default();
    state.update(1.0, 15, true, 1.0);
    assert!((state.multiplier - 0.2).abs() < 1e-6);
    let mut indoors = RainFog::default();
    indoors.update(1.0, 8, true, 1.0);
    assert_eq!(indoors.multiplier, 0.0);
}

#[test]
fn water_vision_ramps_and_brightens_the_water() {
    assert!((water_vision(100) - 0.6).abs() < 1e-6);
    assert_eq!((water_vision(0), water_vision(600), water_vision(9999)), (0.0, 1.0, 1.0));
    let mut a = Attributes::default();
    a.set(Attr::WaterFogEnd, [81.6, 0.0, 0.0, 0.0]);
    let input = FogInputs { fog_type: FogType::Water, water_vision: 1.0, ..inputs() };
    let fog = setup_fog(&a, &input);
    assert_eq!(fog.color[2], 1.0);
    assert!((fog.environmental_end - 81.6).abs() < 1e-4);
    let first = setup_fog(&a, &FogInputs { water_vision: 0.0, ..input });
    assert!((first.environmental_end - 81.6 * 0.25).abs() < 1e-4 && first.color == a.rgb(Attr::WaterFogColor));
}

#[test]
fn facing_the_sunset_tints_the_fog() {
    let mut a = grey(0.5);
    a.set(Attr::SunriseSunsetColor, [1.0, 0.5, 0.0, 1.0]);
    a.set(Attr::SunAngle, [90.0, 0.0, 0.0, 0.0]);
    // The sun sets in the west (-x).
    let west = setup_fog(&a, &FogInputs { forward: [-1.0, 0.0, 0.0], ..inputs() });
    let east = setup_fog(&a, &FogInputs { forward: [1.0, 0.0, 0.0], ..inputs() });
    assert!(west.color[0] > 0.9 && west.color[2] < 0.1, "{west:?}");
    assert_eq!(east.color, [0.5; 3]);
}

#[test]
fn blindness_and_darkness_close_in() {
    let blind = setup_fog(&Attributes::default(), &FogInputs { player: PlayerFog { blindness: Some((100, true)), ..Default::default() }, ..inputs() });
    assert_eq!((blind.environmental_start, blind.environmental_end, blind.sky_end), (1.25, 5.0, 4.0));
    let dark = setup_fog(&Attributes::default(), &FogInputs { player: PlayerFog { darkness: Some(1.0), ..Default::default() }, ..inputs() });
    assert_eq!((dark.environmental_start, dark.environmental_end), (11.25, 15.0));
}

#[test]
fn lava_spectators_see_half_the_render_distance() {
    let lava = FogInputs { fog_type: FogType::Lava, ..inputs() };
    let fire = setup_fog(&grey(0.5), &FogInputs { player: PlayerFog { fire_resistance: true, ..Default::default() }, ..lava });
    assert_eq!((fire.environmental_start, fire.environmental_end), (0.0, 5.0));
    let spectator = setup_fog(&grey(0.5), &FogInputs { player: PlayerFog { spectator: true, fire_resistance: true, ..Default::default() }, ..lava });
    assert_eq!((spectator.environmental_start, spectator.environmental_end), (-8.0, 96.0));
}

#[test]
fn night_vision_brightens_and_a_wither_darkens() {
    let a = grey(0.5);
    let seen = setup_fog(&a, &FogInputs { player: PlayerFog { night_vision: Some(1.0), ..Default::default() }, ..inputs() });
    assert_eq!(seen.color, [1.0; 3]);
    let dark = setup_fog(&a, &FogInputs { player: PlayerFog { night_vision: Some(1.0), darkness: Some(0.0), ..Default::default() }, ..inputs() });
    assert_eq!(dark.color, [0.5; 3]);
    let wither = setup_fog(&a, &FogInputs { player: PlayerFog { darken_world: 1.0, ..Default::default() }, ..inputs() });
    assert!((wither.color[0] - 0.35).abs() < 1e-6 && (wither.color[2] - 0.3).abs() < 1e-6);
}
