//! Pipelined placements (`interact::pipeline`) driving the builder against a toy server with its own world:
//! actions reach it late, some are refused or lost, echoes come back late. The bot sees only its client world
//! (predictions plus echoes), as live.

use std::collections::{HashMap, VecDeque};

use acacia_physics::{BlockPos, Vec3};
use acacia_world::BlockAccess;

use super::build_sim::{builder, items, view};
use super::grid::Grid;
use crate::build::{Block, Structure};
use crate::cadence::splitmix;
use crate::interact::confirm::{Confirm, Expect};
use crate::interact::pipeline::{Pipeline, Undo};
use crate::interact::{block_distance, BLOCK_REACH};
use crate::movement::{Movement, Rollout};
use crate::pathfind::process::{Act, Arbiter, BlockView, Decision, Look, Target};
use crate::pathfind::{answered, NavStatus, SearchJob};
use crate::ActionError;

const ORIGIN: BlockPos = [3, 1, 3];
/// Ticks an action takes to reach the server, and an echo to come back.
const UP: u64 = 2;
const DOWN: u64 = 3;
const TIMEOUT: u64 = 12;
/// The governor's in-flight cap.
const CAP: usize = crate::governor::Limits::DEFAULT_IN_FLIGHT as usize;

struct Toy {
    server: Grid,
    client: Grid,
    to_server: VecDeque<(u64, BlockPos, String)>,
    to_client: VecDeque<(u64, BlockPos, String, String)>,
    rng: u64,
    actions: u32,
    refused: u32,
    lost: u32,
    late: u32,
}

impl Toy {
    fn chance(&mut self, p: f32) -> bool {
        ((splitmix(&mut self.rng) >> 40) as f32 / (1u64 << 24) as f32) < p
    }

    /// The server applies what arrived (losing every 7th action, refusing every 5th: a fixed pattern, so a short
    /// build surely meets both) and echoes it; echoes arrive, some held back.
    fn tick(&mut self, now: u64, pipe: &mut Pipeline<(String, String)>) {
        while self.to_server.front().is_some_and(|(t, ..)| *t <= now) {
            let (_, pos, name) = self.to_server.pop_front().expect("due");
            self.actions += 1;
            if self.actions % 7 == 3 {
                self.lost += 1;
                continue;
            }
            if self.actions % 5 == 1 {
                self.refused += 1;
            } else {
                self.server.set_state(pos, &name, "");
            }
            let (n, p) = view(&self.server, pos);
            let late = if self.chance(0.2) { 4 } else { 0 };
            self.late += u32::from(late > 0);
            let at = (now + DOWN + late).max(self.to_client.back().map_or(0, |(t, ..)| *t));
            self.to_client.push_back((at, pos, n, p));
        }
        while self.to_client.front().is_some_and(|(t, ..)| *t <= now) {
            let (_, pos, name, props) = self.to_client.pop_front().expect("due");
            self.client.set_state(pos, name.trim_start_matches("minecraft:"), &props);
            pipe.echo(pos);
        }
    }
}

struct ClientLook<'a> {
    g: &'a Grid,
    inv: &'a HashMap<String, u32>,
    pipe: &'a Pipeline<(String, String)>,
}

impl Look for ClientLook<'_> {
    fn solid(&self, p: BlockPos) -> Option<bool> {
        self.block(p).map(|b| b.solid())
    }
    fn find(&self, _: &Target) -> Option<Vec3> {
        None
    }
    fn block(&self, p: BlockPos) -> Option<BlockView> {
        self.g.registry.get(self.g.view.block(p[0], p[1], p[2])).map(BlockView::of)
    }
    fn count(&self, item: &str) -> u32 {
        self.inv.get(item).copied().unwrap_or(0)
    }
    fn free_slots(&self) -> u32 {
        20
    }
    fn in_flight(&self, pos: BlockPos) -> bool {
        self.pipe.targets().any(|t| t == pos)
    }
    fn any_in_flight(&self) -> bool {
        !self.pipe.is_empty()
    }
}

/// A plank floor 6×3 with a cobblestone wall row on it.
fn floor_and_wall() -> Structure {
    let mut s = Structure::new([6, 2, 3]);
    for x in 0..6 {
        for z in 0..3 {
            s.set([x, 0, z], Some(Block::new("oak_planks", "")));
            s.set([x, 1, z], Some(Block::new(if z == 0 { "cobblestone" } else { "air" }, "")));
        }
    }
    s
}

#[test]
fn pipelined_placements_build_block_exact_through_refusals_and_losses() {
    let s = floor_and_wall();
    let mut toy = Toy { server: Grid::floor(-20, 30), client: Grid::floor(-20, 30), to_server: VecDeque::new(), to_client: VecDeque::new(), rng: 11, actions: 0, refused: 0, lost: 0, late: 0 };
    let mut inv = items(&[("oak_planks", 64), ("cobblestone", 64), ("dirt", 32)]);
    let mut pipe: Pipeline<(String, String)> = Pipeline::default();
    let mut arb = Arbiter::new();
    let (proc, status) = builder(&s, ORIGIN, &[]);
    let id = arb.add(proc, 10);
    let mut m = Movement::new();
    m.start([0.5, 1.0, 0.5], 0.0, 0.0);
    let (mut most_in_flight, mut finished_in_flight) = (0, None);
    for tick in 0..6000u64 {
        toy.tick(tick, &mut pipe);
        for d in pipe.settle(tick, |p| toy.client.registry.get(toy.client.view.block(p[0], p[1], p[2])).map(|s| (toy.client.view.block(p[0], p[1], p[2]), s))) {
            if let Some(undo) = &d.undo {
                let (name, props) = &undo.extra;
                toy.client.set_state(d.target, name.trim_start_matches("minecraft:"), props);
            }
            let act = Act::Place { item: String::new(), against: [d.target[0], d.target[1] - 1, d.target[2]], face: crate::interact::Face::Up, click: [0.5, 1.0, 0.5] };
            arb.acted(id, &act, &d.result.map_err(ActionError::Rejected));
        }
        let pos = m.position().expect("started");
        let decision = arb.decide(pos, m.on_ground(), &ClientLook { g: &toy.client, inv: &inv, pipe: &pipe });
        match decision {
            // As live, where `place_sent` waits in `admit_action` until the governor lets one more out.
            Decision::Act(..) if pipe.targets().count() >= CAP => m.controls.stop(),
            Decision::Act(pid, Act::Place { item, against, face, click }) => {
                let target = face.adjacent(against);
                let eye = [pos[0], pos[1] + 1.62, pos[2]];
                let before = toy.client.view.block(target[0], target[1], target[2]);
                if block_distance(eye, against) > BLOCK_REACH || inv.get(&item).copied().unwrap_or(0) == 0 {
                    let act = Act::Place { item, against, face, click };
                    arb.acted(pid, &act, &Err(ActionError::NotPossible("out of reach".into())));
                } else {
                    *inv.get_mut(&item).expect("held") -= 1;
                    let name = item.trim_start_matches("minecraft:").to_string();
                    toy.to_server.push_back((tick + UP, target, name.clone()));
                    let old = view(&toy.client, target);
                    toy.client.set_state(target, &name, "");
                    let confirm = Confirm::new(Expect::Solid, Some(before), tick, TIMEOUT, 1).predicted();
                    pipe.push(target, confirm, Undo { replaced: Some(before), extra: old });
                }
            }
            Decision::Act(pid, act) => arb.acted(pid, &act, &Err(ActionError::NotPossible(format!("not modelled: {act:?}")))),
            Decision::Go => {
                let pilot = arb.pilot_mut().expect("routed");
                let opts = pilot.search_opts(Some(super::alter::blocks(64)), 20.0);
                let result = pilot.drive(&toy.client.terrain(), &mut m, opts, &mut |job: SearchJob<Rollout>| answered(job.run(&toy.client.terrain())));
                assert_ne!(result, Ok(NavStatus::Working), "the scene needs no path work");
                if !matches!(result, Ok(NavStatus::Searching | NavStatus::Moving)) {
                    m.controls.stop();
                }
                arb.drove(result, Vec::new());
            }
            _ => m.controls.stop(),
        }
        most_in_flight = most_in_flight.max(pipe.targets().count());
        if arb.outcome(id).is_some() {
            finished_in_flight = Some(pipe.targets().count());
            break;
        }
        m.tick(&toy.client.physics());
    }
    let st = status.lock().unwrap().clone();
    eprintln!("actions {} refused {} lost {} late echoes {} most in flight {most_in_flight} status {st:?}", toy.actions, toy.refused, toy.lost, toy.late);
    assert_eq!(arb.outcome(id), Some(&Ok(())), "status {st:?}");
    assert_eq!(finished_in_flight, Some(0), "finished with placements in flight");
    assert!(toy.refused > 0 && toy.lost > 0 && toy.late > 0, "the server refused, lost and delayed some");
    let to_place = s.cells().filter(|(_, b)| !b.is_air()).count() as u32;
    assert_eq!(st.placed, to_place, "every cell placed once it went through");
    assert_eq!(toy.actions, to_place + toy.refused + toy.lost, "each refused or lost placement sent again");
    assert!((2..=CAP).contains(&most_in_flight), "placements overlapped, within the cap: {most_in_flight}");
    for (p, b) in s.cells() {
        let pos = [ORIGIN[0] + p[0], ORIGIN[1] + p[1], ORIGIN[2] + p[2]];
        let (server, client) = (view(&toy.server, pos), view(&toy.client, pos));
        assert_eq!(server.0, format!("minecraft:{}", b.name.trim_start_matches("minecraft:")), "server at {pos:?}");
        assert_eq!(client, server, "ghost at {pos:?}");
    }
}
