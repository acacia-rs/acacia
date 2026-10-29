//! Whole flights in a synthetic world, the live side simulated: the pilot's orders become the press and the boost
//! a server would confirm, with a rocket's random length.

use std::cell::Cell;

use acacia_physics::{Aabb, BlockPhysics, BlockPos, PlayerState, Vec3, WorldView};

use super::{FlightPilot, FlyOpts, Order, Surface};
use crate::cadence::splitmix;
use crate::movement::{Controls, Rollout};
use crate::pathfind::profile::MovementProfile;

/// Stone up to `ground`, with an optional ridge across x; terrain is known within `seen` blocks of the bot.
struct Land {
    ground: i32,
    ridge: Option<(i32, i32, i32)>,
    seen: f32,
    at: Cell<[f32; 2]>,
}

impl Land {
    fn flat(ground: i32) -> Self {
        Self { ground, ridge: None, seen: 1e9, at: Cell::new([0.0; 2]) }
    }

    fn height(&self, x: i32) -> i32 {
        match self.ridge {
            Some((x0, x1, top)) if (x0..=x1).contains(&x) => top,
            _ => self.ground,
        }
    }
}

impl WorldView for Land {
    fn block_collisions(&self, [x, y, _]: BlockPos, out: &mut Vec<Aabb>) {
        if y <= self.height(x) {
            out.push(Aabb::new(0.0, 0.0, 0.0, 1.0, 1.0, 1.0));
        }
    }

    fn block(&self, [x, y, _]: BlockPos) -> BlockPhysics {
        if y <= self.height(x) { BlockPhysics::solid(0.6) } else { BlockPhysics::AIR }
    }
}

impl Surface for &Land {
    fn top(&mut self, x: i32, z: i32) -> Option<i32> {
        let [ax, az] = self.at.get();
        ((x as f32 - ax).hypot(z as f32 - az) <= self.seen).then(|| self.height(x))
    }

    fn landable(&mut self, x: i32, z: i32) -> bool {
        self.top(x, z).is_some()
    }
}

#[derive(Debug)]
struct Flown {
    feet: Vec3,
    ticks: u64,
    rockets: u32,
    walls: u32,
    /// Fall distance carried into the touchdown.
    fall: f32,
    failed: Option<String>,
}

fn fly(land: &Land, start: Vec3, target: BlockPos, profile: MovementProfile, mut rockets: u32) -> Flown {
    let opts = FlyOpts { profile, ..FlyOpts::default() };
    let mut pilot = FlightPilot::new(target, opts, 7);
    let mut st = PlayerState::new(start);
    st.equipment.elytra = true;
    let mut live = Rollout::new(st);
    let mut c = Controls::default();
    // A new state is not on the ground until a tick finds it there.
    live.step(&c, land);
    let (mut rng, mut walls, mut fall, mut used, mut pressed) = (11u64, 0, 0.0, 0, false);
    for tick in 0..20 * 60 * 5 {
        land.at.set([live.st.pos[0], live.st.pos[2]]);
        let mut surface = land;
        let order = pilot.tick(&live, land, &mut surface, rockets, &mut c);
        let s = &live.st;
        let d = (s.pos[0] - (target[0] as f32 + 0.5)).hypot(s.pos[2] - (target[2] as f32 + 0.5));
        match std::env::var("FLIGHT_TRACE").as_deref() {
            Ok("land") if d < 90.0 => println!(
                "t{tick} d {d:6.2} h {:6.2} hz {:.3} vy {:+.3} pitch {:+6.1} fall {:.2} ground {}",
                s.pos[1] - target[1] as f32, s.vel[0].hypot(s.vel[2]), s.vel[1], c.pitch, s.fall_distance, s.on_ground
            ),
            Ok(v) if v != "land" && tick % 25 == 0 => println!(
                "t{tick} {order:?} pos {:?} vel {:?} ground {} gliding {} jump {} glide {}", s.pos, s.vel, s.on_ground, s.gliding, c.jump, c.glide
            ),
            _ => {}
        }
        match order {
            Order::Fly => {}
            Order::Open => {
                (c.glide, c.jump, pressed) = (true, true, true);
            }
            Order::Rocket => {
                assert!(rockets > 0, "rocket ordered with none left");
                // A flight-1 rocket's boost: 10 * 2 + rand(6) + rand(7) ticks, halved by nothing.
                live.st.glide_boost_ticks = 20 + (splitmix(&mut rng) % 6 + splitmix(&mut rng) % 7) as i64;
                (rockets, used) = (rockets - 1, used + 1);
            }
            Order::Landed => return Flown { feet: live.st.pos, ticks: tick, rockets: used, walls, fall, failed: None },
            Order::Fail(why) => return Flown { feet: live.st.pos, ticks: tick, rockets: used, walls, fall, failed: Some(why) },
        }
        let before = live.st.fall_distance;
        live.step(&c, land);
        if pressed {
            (c.jump, pressed) = (false, false);
        }
        walls += u32::from(live.st.gliding && (live.st.collide_x || live.st.collide_z));
        if live.st.on_ground && fall == 0.0 && tick > 5 {
            fall = before;
        }
    }
    Flown { feet: live.st.pos, ticks: 0, rockets: used, walls, fall, failed: Some("timed out".into()) }
}

fn off(f: &Flown, target: BlockPos) -> f32 {
    (f.feet[0] - (target[0] as f32 + 0.5)).hypot(f.feet[2] - (target[2] as f32 + 0.5))
}

fn assert_landed(f: &Flown, target: BlockPos, within: f32) {
    assert_eq!(f.failed, None, "{f:?}");
    assert_eq!(f.walls, 0, "{f:?}");
    assert!(f.fall <= 3.0, "touchdown fall {} would hurt: {f:?}", f.fall);
    assert!(off(f, target) <= within, "landed {:.2} from the target: {f:?}", off(f, target));
}

#[test]
fn flies_1000_blocks_and_lands_within_3() {
    for profile in [MovementProfile::Optimal, MovementProfile::Human] {
        let land = Land::flat(63);
        let target = [1000, 64, 40];
        let f = fly(&land, [0.5, 64.0, 0.5], target, profile, 64);
        println!("{profile:?}: {f:?}, {:.2} off", off(&f, target));
        assert_landed(&f, target, 3.0);
        assert!(f.rockets <= 40 && f.ticks < 20 * 60, "{f:?}");
    }
}

#[test]
fn climbs_over_a_ridge_seen_late() {
    let mut land = Land::flat(63);
    (land.ridge, land.seen) = (Some((400, 430, 170)), 128.0);
    let target = [700, 64, 0];
    let f = fly(&land, [0.5, 64.0, 0.5], target, MovementProfile::Optimal, 64);
    println!("{f:?}, {:.2} off", off(&f, target));
    assert_landed(&f, target, 3.0);
}

#[test]
fn glides_down_without_rockets_from_high_enough() {
    let land = Land::flat(63);
    let target = [300, 64, 0];
    let f = fly(&land, [0.5, 200.0, 0.5], target, MovementProfile::Optimal, 0);
    println!("{f:?}, {:.2} off", off(&f, target));
    assert_landed(&f, target, 3.0);
}

#[test]
fn refuses_a_glide_that_cannot_reach() {
    let land = Land::flat(63);
    let f = fly(&land, [0.5, 100.0, 0.5], [3000, 64, 0], MovementProfile::Optimal, 0);
    assert!(f.failed.as_deref().is_some_and(|w| w.contains("does not reach")), "{f:?}");
}

#[test]
fn refuses_to_take_off_without_rockets() {
    let land = Land::flat(63);
    let f = fly(&land, [0.5, 64.0, 0.5], [300, 64, 0], MovementProfile::Optimal, 0);
    assert!(f.failed.as_deref().is_some_and(|w| w.contains("no rockets")), "{f:?}");
}
