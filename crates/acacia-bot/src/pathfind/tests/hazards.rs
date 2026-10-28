use std::sync::Arc;

use acacia_physics::{BlockPos, Vec3};

use super::grid::Grid;
use super::search::{ORIGIN, opts};
use crate::pathfind::hazard::{Class, Hazards, Senses, Threat, Threats, View, clear_line};
use crate::pathfind::{Goal, MovementProfile, Path, SearchOpts, search};

pub fn threat(class: Class, feet: Vec3) -> Threat {
    let size = if class == Class::Solid { (1.4, 0.455) } else { (0.6, 1.95) };
    Threat { runtime_id: 1, class, feet, size, motion: [0.0; 3], speed: 0.0, age: 0, lit: false, angry: false }
}

pub fn field(list: Vec<Threat>, profile: MovementProfile) -> Arc<Hazards> {
    Arc::new(Hazards::new(&Threats { list, at: 0 }, profile))
}

fn find(g: &Grid, goal: BlockPos, hazards: Option<Arc<Hazards>>) -> Path {
    let opts = SearchOpts { hazards: hazards.clone(), ..opts(false) };
    search(&g.terrain().with_hazards(hazards), ORIGIN, &Goal::Block(goal), &opts)
}

fn closest(p: &Path, to: Vec3) -> f32 {
    let d = |n: BlockPos| ((n[0] as f32 + 0.5 - to[0]).powi(2) + (n[2] as f32 + 0.5 - to[2]).powi(2)).sqrt();
    p.nodes.iter().map(|n| d(n.pos)).fold(f32::MAX, f32::min)
}

#[test]
fn spheres_fall_off_and_grow_with_threat() {
    let h = field(vec![threat(Class::Melee, [5.5, 1.0, 0.5])], MovementProfile::Optimal);
    let (at, near, far) = (h.cost([5, 1, 0]), h.cost([7, 1, 0]), h.cost([12, 1, 0]));
    assert!(at > near && near > 0.0 && far == 0.0, "{at} {near} {far}");
    let creeper = |lit| Threat { lit, ..threat(Class::Creeper, [5.5, 1.0, 0.5]) };
    let (calm, lit) = (field(vec![creeper(false)], MovementProfile::Optimal), field(vec![creeper(true)], MovementProfile::Optimal));
    assert!(lit.cost([9, 1, 0]) > calm.cost([9, 1, 0]));
    let human = field(vec![threat(Class::Melee, [5.5, 1.0, 0.5])], MovementProfile::Human);
    assert!(human.cost([8, 1, 0]) > h.cost([8, 1, 0]));
    let chasing = Threat { speed: 0.25, ..threat(Class::Melee, [5.5, 1.0, 0.5]) };
    assert!(field(vec![chasing], MovementProfile::Optimal).cost([10, 1, 0]) > 0.0);
}

#[test]
fn edges_pay_every_cell_they_cross() {
    let h = field(vec![threat(Class::Melee, [3.5, 1.0, 0.5])], MovementProfile::Optimal);
    let leap = h.edge([0, 1, 0], [4, 1, 0]);
    let steps: f32 = (1..=4).map(|x| h.cost([x, 1, 0])).sum();
    assert!((leap - steps).abs() < 1e-4, "{leap} {steps}");
    assert_eq!(field(vec![], MovementProfile::Optimal).edge([0, 1, 0], [4, 1, 0]), 0.0);
}

#[test]
fn solid_boxes_close_the_cells_they_reach_into() {
    let h = field(vec![threat(Class::Solid, [4.7, 1.0, 0.5])], MovementProfile::Optimal);
    assert!(h.closed([4, 1, 0]) && h.closed([5, 1, 0]));
    assert!(!h.closed([3, 1, 0]) &&!h.closed([4, 2, 0]) && !h.closed([4, 1, 2]));
    assert_eq!(h.cost([4, 1, 0]), 0.0);
}

#[test]
fn search_detours_around_a_zombie() {
    let g = Grid::floor(-16, 16);
    let zombie = [6.5, 1.0, 0.5];
    let plain = find(&g, [12, 1, 0], None);
    assert!(plain.complete && closest(&plain, zombie) < 1.0);
    let wary = find(&g, [12, 1, 0], Some(field(vec![threat(Class::Melee, zombie)], MovementProfile::Optimal)));
    assert!(wary.complete && closest(&wary, zombie) >= 2.5, "{:?}", wary.nodes);
    // The score bends the route; the planned ticks stay true walking ticks.
    assert!(wary.cost > plain.cost && wary.cost < plain.cost * 2.0, "{} vs {}", wary.cost, plain.cost);
}

/// Step 9's guard. A crowd off the way costs the search nothing (at most 1.2 times the bare walk's nodes); 50 mobs
/// scattered over the way stay a bounded detour; either field builds in well under a millisecond.
#[test]
fn a_crowd_costs_the_search_little() {
    let g = Grid::floor(-24, 40);
    let mut rng = 11;
    let r = |rng: &mut u64, n: u64| (crate::cadence::splitmix(rng) % n) as f32;
    let crowd = |rng: &mut u64, z: (f32, u64)| -> Vec<Threat> {
        (0..50).map(|i| Threat { runtime_id: i, ..threat(Class::Melee, [r(rng, 44) - 4.5, 1.0, z.0 + r(rng, z.1)]) }).collect()
    };
    let (aside, across) = (crowd(&mut rng, (12.5, 8)), crowd(&mut rng, (-19.5, 40)));
    let t = std::time::Instant::now();
    let (aside, across) = (field(aside, MovementProfile::Optimal), field(across, MovementProfile::Optimal));
    let built = t.elapsed() / 2;
    let plain = find(&g, [34, 1, 0], None);
    let (beside, among) = (find(&g, [34, 1, 0], Some(aside)), find(&g, [34, 1, 0], Some(across)));
    eprintln!("crowd: field built in {built:?}; visited bare {}, crowd aside {}, crowd across {}", plain.visited, beside.visited, among.visited);
    assert!(plain.complete && beside.complete && among.complete);
    assert!(beside.visited as f32 <= plain.visited as f32 * 1.2, "aside: {} vs {} bare", beside.visited, plain.visited);
    assert!(among.visited <= 1000, "across: {}", among.visited);
    assert!(built < std::time::Duration::from_millis(1), "{built:?}");
}

#[test]
fn human_notices_late_and_not_behind_a_wall_or_its_back() {
    let g = Grid::floor(-16, 16);
    g.fill([3, 1, -1], [3, 3, 1], "stone");
    let t = g.terrain();
    let sees = |a, b| clear_line(&t, a, b);
    let view = View { eye: [0.5, 2.62, 0.5], yaw: -90.0, health: 20.0, sees: &sees };
    let at = |id, feet| Threat { runtime_id: id, ..threat(Class::Melee, feet) };
    let all = |tick| Threats { list: vec![at(1, [6.5, 1.0, 0.5]), at(2, [6.5, 1.0, 4.5]), at(3, [-6.5, 1.0, 0.5]), at(4, [-2.5, 1.0, 0.5])], at: tick };
    let ids = |t: Threats| t.list.iter().map(|t| t.runtime_id).collect::<Vec<_>>();
    assert_eq!(ids(Senses::new(1).filter(all(0), &view, MovementProfile::Optimal)), [1, 2, 3, 4]);
    let mut human = Senses::new(1);
    assert!(ids(human.filter(all(0), &view, MovementProfile::Human)).is_empty(), "reacts only after a delay");
    assert_eq!(ids(human.filter(all(10), &view, MovementProfile::Human)), [2, 4], "behind the wall and its back unseen; earshot heard");
}

#[test]
fn a_boat_in_a_corridor_blocks_it() {
    let g = Grid::floor(-16, 16);
    g.fill([-1, 1, 1], [12, 2, 1], "stone");
    g.fill([-1, 1, -1], [12, 2, -1], "stone");
    let boat = Some(field(vec![threat(Class::Solid, [6.5, 1.0, 0.5])], MovementProfile::Optimal));
    assert!(find(&g, [10, 1, 0], None).complete);
    let p = find(&g, [10, 1, 0], boat);
    assert!(p.nodes.iter().all(|n| !(5..=7).contains(&n.pos[0]) || n.pos[2] != 0 || n.pos[1] != 1), "{:?}", p.nodes);
}
