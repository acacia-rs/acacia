//! The planner among mobs (milestone M4), both profiles, in the headless entity simulation.

use super::drive::standing;
use super::entity_sim::{Brain, Crowd, Harm, Mob};
use super::grid::Grid;
use super::pilot_sim::{Flight, Sky, fly_with};
use crate::pathfind::{Goal, GotoOpts, MovementProfile, SearchOpts};

const PROFILES: [MovementProfile; 2] = [MovementProfile::Optimal, MovementProfile::Human];

pub fn goto_opts(profile: MovementProfile, seed: u64) -> GotoOpts {
    let search = SearchOpts { profile, max_nodes: 20_000, ..SearchOpts::default() };
    GotoOpts { search, seed: Some(seed), give_up_ticks: Some(400), ..GotoOpts::default() }
}

/// Flies from the origin to `goal` among `mobs`; returns the flight and the harm taken.
pub fn among(g: &Grid, mobs: Vec<Mob>, goal: Goal, profile: MovementProfile, seed: u64, max_ticks: u32) -> (Flight, Harm) {
    let mut crowd = Crowd::new(g, mobs);
    let sky = Sky { horizon: None, delay: 0, max_ticks };
    let f = fly_with(g, &mut standing([0.5, 1.0, 0.5]), goal, goto_opts(profile, seed), sky, &mut |t, feet| crowd.step(t, feet));
    (f, crowd.harm)
}

#[test]
fn walks_round_a_field_of_standing_zombies() {
    let g = Grid::floor(-24, 40);
    let spots = [[8.5, 1.0, 0.5], [14.5, 1.0, 3.5], [14.5, 1.0, -3.5], [20.5, 1.0, 0.5], [26.5, 1.0, -1.5]];
    for profile in PROFILES {
        let mobs = spots.iter().map(|&p| Mob::new("zombie", p, Brain::Still)).collect();
        let (f, harm) = among(&g, mobs, Goal::Block([34, 1, 0]), profile, 7, 1200);
        for z in spots {
            let (t, d) = f.track.iter().enumerate().map(|(t, p)| (t, (p[0] - z[0]).hypot(p[2] - z[2]))).fold((0, f32::MAX), |a, b| if b.1 < a.1 { b } else { a });
            eprintln!("{profile:?} zombie {z:?}: closest {d:.2} at tick {t} {:?}", f.track[t]);
        }
        assert_eq!(f.result, Ok(()), "{profile:?}");
        assert_eq!(harm, Harm::default(), "{profile:?}");
    }
}

/// A chasing zombie placed by `seed` somewhere along a 40-block walk.
fn zombie_run(profile: MovementProfile, seed: u64) -> (Flight, Harm) {
    let g = Grid::floor(-24, 46);
    let mut rng = seed;
    let r = |rng: &mut u64, n: u64| (crate::cadence::splitmix(rng) % n) as f32;
    let at = [10.5 + r(&mut rng, 20), 1.0, r(&mut rng, 17) - 7.5];
    among(&g, vec![Mob::new("zombie", at, Brain::Chase)], Goal::Block([40, 1, 0]), profile, seed, 1500)
}

fn zombie_runs(seeds: std::ops::Range<u64>) {
    for profile in PROFILES {
        for seed in seeds.clone() {
            let (f, harm) = zombie_run(profile, seed);
            assert_eq!(f.result, Ok(()), "{profile:?} seed {seed}");
            assert_eq!(harm.melee, 0, "{profile:?} seed {seed}: hit");
        }
    }
}

/// Seed 6 puts the zombie dead ahead of a long shortcut, which holds the follower's index at its start: the alarm
/// must watch the nodes ahead of the bot, not of that index.
#[test]
fn re_plans_for_a_zombie_ahead_of_a_running_shortcut() {
    zombie_runs(6..7);
}

#[test]
fn outpaces_a_chasing_zombie() {
    zombie_runs(0..2);
}

#[test]
#[ignore = "slow in debug: cargo test --release -- --ignored zombie_runs_20"]
fn zombie_runs_20() {
    zombie_runs(0..20);
}

#[test]
fn crosses_a_lane_a_zombie_paces() {
    let g = Grid::floor(-24, 46);
    g.fill([-2, 1, 4], [36, 2, 4], "stone").fill([-2, 1, -4], [36, 2, -4], "stone");
    for profile in PROFILES {
        let mob = Mob::new("zombie", [16.5, 1.0, -2.5], Brain::Pace([16.5, 1.0, -2.5], [16.5, 1.0, 3.5]));
        let (f, harm) = among(&g, vec![mob], Goal::Block([32, 1, 0]), profile, 5, 1500);
        assert_eq!(f.result, Ok(()), "{profile:?}");
        assert_eq!(harm.melee, 0, "{profile:?}");
    }
}

#[test]
fn passes_a_skeleton_taking_at_most_two_arrows() {
    let g = Grid::floor(-24, 46);
    for profile in PROFILES {
        let (f, harm) = among(&g, vec![Mob::new("skeleton", [16.5, 1.0, 8.5], Brain::Still)], Goal::Block([34, 1, 0]), profile, 9, 1500);
        assert_eq!(f.result, Ok(()), "{profile:?}");
        assert!(harm.arrows <= 2 && harm.melee == 0, "{profile:?}: {harm:?}");
    }
}

/// Step 9's swarm guard: 8 bots' gotos (seeds) across a field of 40 cows and 10 standing zombies all arrive unhit,
/// at a bounded cost per driven tick (searches included).
#[test]
#[ignore = "timing: cargo test --release -- --ignored eight_bots_among_fifty_mobs"]
fn eight_bots_among_fifty_mobs() {
    let g = Grid::floor(-24, 46);
    let mut rng = 3;
    let r = |rng: &mut u64, n: u64| (crate::cadence::splitmix(rng) % n) as f32;
    let spots: Vec<_> = (0..50).map(|_| [r(&mut rng, 36) + 4.5, 1.0, r(&mut rng, 40) - 19.5]).collect();
    let (mut ticks, t) = (0, std::time::Instant::now());
    for seed in 0..8 {
        let profile = PROFILES[seed as usize % 2];
        let mobs = spots.iter().enumerate().map(|(i, &p)| Mob::new(if i % 5 == 0 { "zombie" } else { "cow" }, p, Brain::Still)).collect();
        let (f, harm) = among(&g, mobs, Goal::Block([42, 1, 0]), profile, seed, 1500);
        assert_eq!(f.result, Ok(()), "{profile:?} seed {seed}");
        assert_eq!(harm.melee, 0, "{profile:?} seed {seed}");
        ticks += f.ticks;
    }
    let per_tick = t.elapsed() / ticks;
    eprintln!("8 bots among 50 mobs: {ticks} ticks, {per_tick:?} a tick");
    assert!(per_tick < std::time::Duration::from_millis(2), "{per_tick:?} a tick");
}

/// A boat across a 1-wide, 2-high corridor closed behind the bot (too low to step onto it), gone after `leaves` ticks (never: `None`).
fn boat_in_corridor(profile: MovementProfile, leaves: Option<u32>) -> Flight {
    let g = Grid::floor(-8, 24);
    g.fill([-1, 1, -1], [20, 3, 1], "stone").fill([0, 1, 0], [20, 2, 0], "air").fill([4, 3, 0], [12, 3, 0], "stone");
    let mut crowd = Crowd::new(&g, vec![Mob::new("boat", [8.7, 1.0, 0.5], Brain::Still)]);
    let sky = Sky { horizon: None, delay: 0, max_ticks: 1200 };
    let mut scene = |t: u32, feet| {
        if leaves == Some(t) {
            crowd.mobs.clear();
        }
        crowd.step(t, feet)
    };
    fly_with(&g, &mut standing([0.5, 1.0, 0.5]), Goal::Block([16, 1, 0]), goto_opts(profile, 4), sky, &mut scene)
}

#[test]
fn waits_for_a_boat_to_clear_a_corridor_then_fails_if_it_stays() {
    for profile in PROFILES {
        assert_eq!(boat_in_corridor(profile, Some(60)).result, Ok(()), "{profile:?}: waited, then went on");
        let stuck = boat_in_corridor(profile, None);
        let err = stuck.result.expect_err("a boat that never moves");
        assert!(err.contains("cannot reach") && stuck.ticks > 100, "{profile:?}: waited before failing: {err} after {}", stuck.ticks);
    }
}

#[test]
fn walks_through_a_cow_in_a_corridor() {
    let g = Grid::floor(-8, 24);
    g.fill([-1, 1, -1], [20, 2, 1], "stone").fill([0, 1, 0], [20, 2, 0], "air");
    for profile in PROFILES {
        let (f, _) = among(&g, vec![Mob::new("cow", [8.5, 1.0, 0.5], Brain::Still)], Goal::Block([16, 1, 0]), profile, 3, 800);
        assert_eq!(f.result, Ok(()), "{profile:?}: a cow is pushed aside, never a wall");
    }
}
