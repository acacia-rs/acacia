//! The planner's simulation against the live one: forks step bit-exact on the search's terrain, and a
//! state rebuilt from quantized momentum stays close to the real one.

use super::grid::Grid;
use crate::movement::{Controls, Movement, Sim};
use crate::pathfind::Momentum;

/// Fidelity bound for a synthesized state (plan: 1/512 block over 30 ticks).
const SYNTH_TOLERANCE: f32 = 1.0 / 512.0;

/// A step, a slab, a wall and a hole, so the script bumps, climbs and falls.
fn course() -> Grid {
    let g = Grid::floor(-30, 40);
    g.fill([6, 1, -3], [6, 1, 3], "stone");
    g.fill_state([10, 1, -3], [10, 1, 3], "smooth_stone_slab", "minecraft:vertical_half=bottom");
    g.fill([-8, 1, -8], [-8, 3, 8], "stone");
    g.fill([3, 0, 5], [4, 0, 6], "air");
    g
}

/// Controls for tick `t`: a forward tap and double tap, a sprint, bunny hops while turning, sneaking,
/// strafing, a key sprint into a wall, walking back and a standing jump.
fn script(t: u32) -> Controls {
    let mut c = Controls { yaw: -90.0, ..Controls::default() };
    match t {
        5..7 | 9..50 => c.forward = 1.0,
        50..80 => {
            (c.forward, c.sprint, c.jump) = (1.0, true, true);
            c.yaw = -90.0 + (t - 50) as f32 * 6.0;
        }
        80..110 => (c.forward, c.strafe, c.sneak, c.yaw) = (1.0, 1.0, true, 90.0),
        110..150 => (c.forward, c.strafe, c.sprint, c.yaw) = (1.0, -1.0, true, 100.0),
        150..170 => (c.forward, c.yaw) = (-1.0, 45.0),
        185..188 => c.jump = true,
        _ => {}
    }
    c.jump |= matches!(t, 20..23 | 33..35);
    c
}

#[test]
fn forks_step_bit_exact_with_the_live_simulation() {
    let g = course();
    let (world, terrain) = (g.physics(), g.terrain());
    let mut m = Movement::new();
    m.start([0.5, 1.0, 0.5], 0.0, 0.0);
    let mut fork = None;
    for t in 0..208 {
        // Forked inside the double-tap window, so the key latches must carry over.
        if t == 8 {
            fork = m.fork();
        }
        m.controls = script(t);
        m.tick(&world);
        if let Some(r) = &mut fork {
            r.step(&script(t), &terrain);
            assert_eq!(m.state(), Some(&r.st), "tick {t}");
        }
    }
    let end = m.position().unwrap();
    assert!(end[0].abs() + end[2].abs() > 5.0, "the script moved: {end:?}");
}

#[test]
fn synthesized_momentum_tracks_the_real_state() {
    let g = course();
    let (world, terrain) = (g.physics(), g.terrain());
    let mut m = Movement::new();
    m.start([0.5, 1.0, 0.5], 0.0, 0.0);
    let mut worst = 0.0f32;
    for t in 0..178 {
        if t % 4 == 0 {
            let mut real = m.fork().unwrap();
            let p = real.st.pos;
            let at = [p[0].floor() as i32, (p[1] + 1e-3).floor() as i32, p[2].floor() as i32];
            let mut synth = real.clone();
            synth.st = Momentum::snap(&real.motion(), at).synth(at, &real).st;
            for k in 0..30 {
                real.step(&script(t + k), &terrain);
                synth.step(&script(t + k), &terrain);
                let err = (0..3).map(|i| (real.st.pos[i] - synth.st.pos[i]).abs()).fold(0.0, f32::max);
                worst = worst.max(err);
                assert!(err <= SYNTH_TOLERANCE, "snap at {t}, tick {k}: off by {err}; real {:?}, synth {:?}", real.st.pos, synth.st.pos);
            }
        }
        m.controls = script(t);
        m.tick(&world);
    }
    eprintln!("worst synth error {worst}");
}
