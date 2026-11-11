use acacia_registry::Block;

use super::*;
use crate::collide::Blocks;

/// Stone at y -1 where `x < 10`, a lava lake at y -1 beyond.
struct Ground;

impl Blocks for Ground {
    fn state(&self, x: i32, y: i32, _z: i32) -> Option<u32> {
        let block = match y {
            -1 if x < 10 => Block::Stone,
            -2 | -1 => Block::Lava,
            _ => Block::Air,
        };
        Some(block.default_state().id())
    }

    fn min_y(&self) -> i32 {
        -64
    }
}

const AHEAD: Rider = Rider { xxa: 0.0, zza: 0.98, jumping: false, sprinting: false, yaw: 0.0, pitch: 0.0 };

fn mount(name: &str, size: (f32, f32), pos: [f64; 3]) -> Mount {
    let mut mount = Mount::new(name, size, [0.0, size.1, 0.0], pos, 0.0).expect("drivable");
    for _ in 0..3 {
        mount.tick(&Ground, Rider::default());
    }
    mount
}

#[test]
fn a_horse_reaches_its_ground_speed() {
    let mut horse = mount("horse", (1.3964844, 1.6), [0.5, 0.0, 0.5]);
    let mut step = 0.0;
    for _ in 0..60 {
        let z = horse.body.pos[2];
        horse.tick(&Ground, AHEAD);
        step = horse.body.pos[2] - z;
    }
    let accel = 0.225_f32 as f64 * 0.98_f32 as f64;
    let friction = (0.6_f32 * 0.91_f32) as f64;
    assert!((step - accel / (1.0 - friction)).abs() < 1.0E-6, "{step}");
    assert_eq!(horse.body.pos[0], 0.5);
}

#[test]
fn a_pig_walks_on_with_no_keys() {
    let mut pig = mount("pig", (0.9, 0.9), [0.5, 0.0, 0.5]);
    for _ in 0..10 {
        pig.tick(&Ground, Rider { yaw: 90.0, ..Rider::default() });
    }
    assert!(pig.body.pos[0] < 0.0, "pig at {:?}", pig.body.pos);
    assert_eq!(pig.body.yaw, 90.0);
}

#[test]
fn a_happy_ghast_hovers() {
    let mut ghast = mount("happy_ghast", (4.0, 4.0), [0.5, 5.0, 0.5]);
    for _ in 0..20 {
        ghast.tick(&Ground, Rider::default());
    }
    assert_eq!(ghast.body.pos[1], 5.0);
}

#[test]
fn a_strider_stands_on_lava() {
    let mut strider = mount("strider", (0.9, 1.7), [20.5, 1.0, 0.5]);
    for _ in 0..40 {
        strider.tick(&Ground, Rider::default());
    }
    assert!((strider.body.pos[1] + 0.5).abs() < 1.0E-6, "strider at {}", strider.body.pos[1]);
    assert!(strider.body.on_ground);
}

#[test]
fn a_full_charge_horse_jump_leaves_at_its_strength() {
    let mut horse = mount("horse", (1.3964844, 1.6), [0.5, 0.0, 0.5]);
    horse.on_player_jump(100);
    assert!(horse.standing);
    let ground = horse.body.pos[1];
    horse.tick(&Ground, Rider::default());
    let rise = horse.body.pos[1] - ground;
    assert!((rise - 0.7_f32 as f64).abs() < 1.0E-6, "{rise}");
}

#[test]
fn the_ghast_seats_its_driver_in_front() {
    let mut ghast = Mount::new("happy_ghast", (4.0, 4.0), [0.0, 4.0, 1.7], [0.0; 3], 0.0).unwrap();
    ghast.body.yaw = 90.0;
    let feet = ghast.rider_feet();
    assert!((feet[0] + 1.7).abs() < 1.0E-4 && feet[2].abs() < 1.0E-4, "{feet:?}");
    assert!((feet[1] - 3.4).abs() < 1.0E-9);
}
