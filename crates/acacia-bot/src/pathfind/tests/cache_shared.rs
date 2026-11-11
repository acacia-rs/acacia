//! One rollout cache shared by searches running at once (a plan-ahead in the background and a re-plan).

use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Barrier, Mutex};

use acacia_physics::BlockPos;
use acacia_world::ChunkView;

use super::drive::standing;
use super::perf::{Case, cases, run};
use crate::movement::Rollout;
use crate::pathfind::{Blocks, Path, RolloutCache, SearchJob, Terrain};

fn case(name: &str) -> Case {
    cases().into_iter().find(|c| c.name == name).expect("perf case")
}

fn cells(p: &Path) -> Vec<(BlockPos, crate::pathfind::MoveKind)> {
    p.nodes.iter().map(|n| (n.pos, n.kind)).collect()
}

/// The grid, holding its first read until told to go on: a search started on it is under way, not done.
struct Gate<'a> {
    view: &'a ChunkView,
    hold: Mutex<Option<(Sender<()>, Receiver<()>)>>,
}

impl Blocks for Gate<'_> {
    fn ids(&self, p: BlockPos) -> Option<(u32, u32)> {
        if let Some((started, go)) = self.hold.lock().expect("gate").take() {
            started.send(()).expect("test waits");
            go.recv().expect("test releases");
        }
        self.view.ids(p)
    }
}

/// A plan-ahead under way in the background: a re-plan meanwhile still answers from the cache (it used to search
/// without it while the plan-ahead held the lock).
#[test]
fn a_replan_during_a_plan_ahead_hits_the_cache() {
    let c = case("gap course Human");
    let g = (c.grid)();
    let cache = Arc::new(RolloutCache::default());
    let (warm, _) = run(&c, &g, &cache);
    let ((started_tx, started_rx), (go_tx, go_rx)) = (channel(), channel());
    let gate = Gate { view: &g.view, hold: Mutex::new(Some((started_tx, go_rx))) };
    let job = SearchJob { from: Rollout::new(standing(c.from).state().cloned().expect("started")), goal: c.goal.clone(), opts: c.opts.clone(), cache: cache.clone() };
    std::thread::scope(|s| {
        let ahead = s.spawn(|| job.run(&Terrain::new(&gate, &g.registry)));
        started_rx.recv().expect("plan-ahead started");
        let (replan, _) = run(&c, &(c.grid)(), &cache);
        go_tx.send(()).expect("plan-ahead waits");
        let ahead = ahead.join().expect("plan-ahead");
        let r = replan.macro_stats;
        assert!(r.hits > 0 && r.rollouts == 0, "re-plan during the plan-ahead: {r:?}");
        assert_eq!(cells(&replan), cells(&warm));
        assert_eq!(cells(&ahead), cells(&warm));
    });
}

/// Searches filling one cold cache at once plan what each plans alone.
#[test]
fn concurrent_searches_share_one_cache() {
    let c = case("gap course Optimal");
    let g = (c.grid)();
    let (alone, _) = run(&c, &g, &RolloutCache::disabled());
    let cache = RolloutCache::default();
    let start = Barrier::new(4);
    std::thread::scope(|s| {
        let runs: Vec<_> = (0..4).map(|_| s.spawn(|| {
            let g = (c.grid)();
            start.wait();
            run(&c, &g, &cache).0
        })).collect();
        for p in runs.into_iter().map(|r| r.join().expect("search")) {
            assert_eq!((p.complete, cells(&p)), (alone.complete, cells(&alone)));
            assert!((p.cost - alone.cost).abs() < 0.01, "{} vs {}", p.cost, alone.cost);
        }
    });
}
