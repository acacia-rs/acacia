//! The redstone tier's scenes built in the simulation (`redstone_sim` as the server).

use acacia_physics::BlockPos;
use acacia_world::BlockAccess;

use super::build_sim::{builder, explain, run, World};
use super::build_tests::{scaffold_left, ORIGIN};
use super::grid::Grid;
use crate::build::tests::scenes_redstone::{self, FLOORS, STOCK};
use crate::build::{check, Structure, Transform};
use crate::pathfind::process::{Arbiter, BlockView};

fn build(s: &Structure, floor: &str) {
    let g = Grid::floor(-20, 30);
    g.fill(ORIGIN, [ORIGIN[0] + s.size[0] - 1, ORIGIN[1], ORIGIN[2] + s.size[2] - 1], floor);
    let mut stock: Vec<(&str, u32)> = Vec::new();
    // The kit's dirt and hoe, as examples/support/house.rs `kit` gives them.
    for (item, n) in STOCK.iter().chain(&[("dirt", 64), ("diamond_hoe", 1)]) {
        match stock.iter_mut().find(|(i, _)| i == item) {
            Some((_, c)) => *c += n,
            None => stock.push((item, *n)),
        }
    }
    let mut w = World::new(&g, &stock);
    let (proc, status) = builder(s, ORIGIN, &[]);
    let mut arb = Arbiter::new();
    let id = arb.add(proc, 10);
    let ticks = run(&mut w, [0.5, 1.0, 0.5], &mut arb, id, 12_000);
    let st = status.lock().unwrap().clone();
    eprintln!("{ticks} ticks, {st:?}");
    explain(&w, s, ORIGIN).iter().for_each(|l| eprintln!("OPEN {l}"));
    let at = Transform { origin: ORIGIN, ..Transform::default() };
    let wrong = check(s, &at, |p| g.registry.get(g.view.block(p[0], p[1], p[2])).map(BlockView::of));
    assert_eq!(arb.outcome(id), Some(&Ok(())), "status {st:?}");
    assert_eq!(wrong, Vec::new());
    assert_eq!(scaffold_left(&g), Vec::<BlockPos>::new());
}

#[test]
fn builds_rails_in_an_order_that_gives_their_shapes() {
    build(&scenes_redstone::rails(), FLOORS[0].1);
}

#[test]
fn builds_a_garden_tilling_and_pouring() {
    build(&scenes_redstone::garden(), FLOORS[1].1);
}
