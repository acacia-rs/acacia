//! The schematic builder as a process: each tick it re-surveys the footprint and picks one thing to do. Design:
//! acacia-pathfind/docs/builder.md.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use acacia_physics::BlockPos;

use super::job::BuildJob;
use super::plan;
use super::status::{BuildStatus, Phase, StatusHandle};
use super::structure::{Block, Structure, StructureError};
use super::survey::{need_with_text, Need};
use crate::pathfind::process::Look;
use crate::ActionError;

/// Failed acts on one cell before it is given up.
const MAX_FAILS: u32 = 4;
/// Placements that came out wrong (and were broken again) before a cell is given up.
const MAX_PLACES: u32 = 3;
const RETRY_TICKS: u64 = 20;
/// Ticks a stand the pilot failed to reach is left out.
const AVOID_TICKS: u64 = 400;
const REST_TICKS: u64 = 40;
/// Rests (40 ticks each) before a cell nothing reaches is given up: a minute and a half.
const MAX_RESTS: u32 = 45;
/// Times a cell given up for want of a stand is tried again once nothing else can be done (the build around it
/// changes what reaches it).
const MAX_REVIVES: u32 = 3;

pub(super) struct Cell {
    pub pos: BlockPos,
    pub target: Block,
    /// A sign's text to write.
    pub text: Option<super::structure::SignText>,
}

#[derive(Default)]
pub(super) struct Tries {
    pub fails: u32,
    pub places: u32,
    /// Placements counted as adding to the cell (`rules::more_placements`), not as placing it again.
    pub adds: u32,
    /// Times nowhere was found to do it from.
    pub rests: u32,
    pub revives: u32,
    pub next_at: u64,
}

impl Tries {
    fn given_up(&self) -> bool {
        self.why().is_some()
    }

    fn why(&self) -> Option<&'static str> {
        match () {
            _ if self.fails >= MAX_FAILS => Some("acts failed"),
            _ if self.places > MAX_PLACES => Some("placed wrong"),
            _ if self.rests > MAX_RESTS => Some("no stand"),
            _ => None,
        }
    }
}

pub(super) struct Station {
    pub pos: BlockPos,
    /// Items it was found without.
    pub lacks: HashSet<String>,
}

/// A restock in flight: the counts before it, to learn what the station lacks.
pub(super) struct Restock {
    pub station: usize,
    pub before: Vec<(String, u32)>,
}

pub struct Builder {
    pub(super) job: BuildJob,
    pub(super) cells: Vec<Cell>,
    pub(super) index: HashMap<BlockPos, usize>,
    pub(super) tries: HashMap<BlockPos, Tries>,
    /// Temporary blocks: pillars and supports, removed at the end.
    pub(super) scaffold: HashSet<BlockPos>,
    /// Blocks holding liquids back from outside the footprint: left in place.
    pub(super) seals: HashSet<BlockPos>,
    pub(super) stations: Vec<Station>,
    pub(super) restock: Option<Restock>,
    /// The goal being driven to, kept while its task is open (a new pilot each tick would never get there).
    pub(super) commit: Option<(BlockPos, crate::pathfind::Goal)>,
    /// The goal the pilot reached: asking for it again would only hold still (the arbiter keeps an arrived route).
    pub(super) arrived: Option<crate::pathfind::Goal>,
    /// (task, stand) pairs tried since the last act (`arrival::missed`); `settling`: the stand driven to its centre.
    pub(super) tried: Vec<(BlockPos, BlockPos)>,
    pub(super) settling: Option<BlockPos>,
    /// Stand feet the pilot failed to reach, until the tick.
    pub(super) avoid: HashMap<BlockPos, u64>,
    /// Cells with a thin target (`rules::thin`): the pilot keeps the body out of them (`SearchOpts::no_body`).
    pub(super) thin: Arc<HashSet<BlockPos>>,
    /// Walks failed in a row (an arrival or an act resets it); see `process::opts`.
    pub(super) failed_walks: u32,
    /// Cells already built (a block target that matches): the pilot's own work leaves them alone
    /// (`SearchOpts::keep`). Built cells only: keeping every block target cost one bot 267 s against 200 s
    /// (live house16), its paths walking round unbuilt floor instead of over scaffold there.
    pub(super) keep: Arc<HashSet<BlockPos>>,
    pub(super) tick: u64,
    pub(super) waiting_since: Option<u64>,
    /// Cells to claim on the board for the step just planned (`swarm`).
    pub(super) batch: Vec<BlockPos>,
    /// A placement to make on the way, offered with this tick's walk (`en_route`).
    pub(super) en_route: Option<crate::pathfind::process::Act>,
    pub(super) swarm: Option<super::swarm::Swarm>,
    pub(super) journal: Option<Arc<super::journal::Journal>>,
    status: StatusHandle,
}

#[derive(Debug, thiserror::Error)]
pub enum BuildError {
    #[error("reading {0}: {1}")]
    Io(String, std::io::Error),
    #[error(transparent)]
    Structure(#[from] StructureError),
}

impl Builder {
    /// Loads the job's structure file.
    pub fn new(job: BuildJob) -> Result<Self, BuildError> {
        let bytes = std::fs::read(&job.structure).map_err(|e| BuildError::Io(job.structure.display().to_string(), e))?;
        Ok(Self::with_structure(job, &Structure::load(&bytes)?))
    }

    /// Builds `structure` where `job` says (its file is not read).
    pub fn with_structure(job: BuildJob, structure: &Structure) -> Self {
        let cells: Vec<Cell> = structure
            .cells()
            .filter(|(_, b)| b.name != "minecraft:structure_void")
            .map(|(local, b)| Cell { pos: job.at.apply(structure.size, local), target: job.at.block(b), text: structure.text(local).cloned() })
            .collect();
        let index = cells.iter().enumerate().map(|(i, c)| (c.pos, i)).collect();
        let thin = Arc::new(cells.iter().filter(|c| super::rules::thin(&c.target.name)).map(|c| c.pos).collect());
        let stations = job.stations.iter().map(|&pos| Station { pos, lacks: HashSet::new() }).collect();
        let status = Arc::new(Mutex::new(BuildStatus { cells: cells.len(), ..BuildStatus::default() }));
        Self {
            batch: Vec::new(),
            en_route: None,
            swarm: None,
            journal: None,
            job,
            cells,
            index,
            tries: HashMap::new(),
            scaffold: HashSet::new(),
            seals: HashSet::new(),
            stations,
            restock: None,
            commit: None,
            arrived: None,
            tried: Vec::new(),
            settling: None,
            avoid: HashMap::new(),
            thin,
            failed_walks: 0,
            keep: Arc::default(),
            tick: 0,
            waiting_since: None,
            status,
        }
    }

    pub fn job(&self) -> &BuildJob {
        &self.job
    }

    /// Progress, readable while the builder runs inside [`crate::pathfind::Processes`].
    pub fn status(&self) -> StatusHandle {
        self.status.clone()
    }

    /// Why each open placement is not being done from `feet` (tests).
    #[cfg(test)]
    pub(crate) fn explain(&self, look: &dyn Look, feet: acacia_physics::Vec3) -> Vec<String> {
        plan::explain(self, look, feet)
    }

    /// Diagnostics: each open placement in `bot`'s world, with the click it has from where the bot stands and the
    /// stands it could use (a builder for the same job, outside the processes).
    pub fn explain_for(&self, bot: &crate::Bot) -> Vec<String> {
        let Some(feet) = bot.movement().and_then(|m| m.position()) else { return Vec::new() };
        plan::explain(self, &crate::pathfind::process::run::BotLook(bot), feet)
    }

    pub(super) fn target(&self, pos: BlockPos) -> Option<&Block> {
        self.index.get(&pos).map(|&i| &self.cells[i].target)
    }

    /// A cell to leave alone for now: given up on, or waiting out a retry.
    pub(super) fn resting(&self, pos: BlockPos) -> bool {
        self.tries.get(&pos).is_some_and(|t| t.given_up() || t.next_at > self.tick)
    }

    /// Nowhere to do the cell from yet: let others go first for a while (not a failure).
    pub(super) fn rest(&mut self, pos: BlockPos) {
        let t = self.tries.entry(pos).or_default();
        (t.rests, t.next_at) = (t.rests + 1, self.tick + REST_TICKS);
        self.note(super::journal::Entry::Rest { cell: pos });
        let t = &self.tries[&pos];
        if t.rests == MAX_RESTS + 1 {
            tracing::debug!(?pos, "build cell given up: no stand");
        }
    }

    /// Feet in `cell` are not used: next to another bot, or a stand the pilot recently failed to reach.
    pub(super) fn shunned(&self, cell: BlockPos) -> bool {
        self.crowded(cell) || self.avoid.get(&cell).is_some_and(|t| *t > self.tick)
    }

    /// The pilot could not get to `goal` (a refused pillar, no route): its stands are left out for a while and
    /// the task rests (a revivable give-up) instead of failing; the cell itself was never tried.
    pub(super) fn unreached(&mut self, task: BlockPos, goal: &crate::pathfind::Goal) {
        let until = self.tick + AVOID_TICKS;
        let mut mark = |g: &crate::pathfind::Goal| {
            if let crate::pathfind::Goal::Block(p) = g {
                self.avoid.insert(*p, until);
            }
        };
        match goal {
            crate::pathfind::Goal::Or(gs) => gs.iter().for_each(&mut mark),
            g => mark(g),
        }
        self.avoid.retain(|_, t| *t > self.tick);
        self.rest(task);
    }

    /// Clears the rests of cells given up only for want of a stand; false: none left to try again.
    pub(super) fn revive(&mut self) -> bool {
        let mut any = false;
        for (pos, t) in self.tries.iter_mut().filter(|(_, t)| t.rests > MAX_RESTS && t.fails < MAX_FAILS && t.places <= MAX_PLACES && t.revives < MAX_REVIVES) {
            tracing::debug!(?pos, revives = t.revives, "build cell tried again");
            (t.rests, t.revives, any) = (0, t.revives + 1, true);
        }
        any
    }

    pub(super) fn given_up(&self, pos: BlockPos) -> bool {
        self.tries.get(&pos).is_some_and(Tries::given_up)
    }

    pub(super) fn fail(&mut self, pos: BlockPos, e: &ActionError) {
        tracing::debug!(?pos, %e, "build act failed");
        let t = self.tries.entry(pos).or_default();
        (t.fails, t.next_at) = (t.fails + 1, self.tick + RETRY_TICKS);
        if self.commit.as_ref().is_some_and(|(p, _)| *p == pos) {
            self.commit = None;
        }
    }

    pub(super) fn survey(&self, look: &dyn Look) -> Vec<Need> {
        let mut needs: Vec<Need> = self.cells.iter().map(|c| need_with_text(&c.target, c.text.as_ref(), c.pos, look, self.job.fill_only)).collect();
        super::pairs::settle(&self.cells, &self.index, look, self.job.fill_only, &mut needs);
        needs
    }

    /// Learns what a station lacks from what the last restock brought.
    pub(super) fn check_restock(&mut self, look: &dyn Look) {
        let Some(r) = self.restock.take() else { return };
        for (item, before) in r.before {
            if look.count(&item) <= before {
                self.stations[r.station].lacks.insert(item);
            }
        }
    }

    pub(super) fn publish(&self, needs: &[Need], look: &dyn Look, phase: Phase, missing: Vec<String>) {
        let Ok(mut s) = self.status.lock() else { return };
        let count = |n: Need| needs.iter().filter(|x| **x == n).count();
        (s.done, s.to_place, s.to_break, s.unknown, s.kept) = (count(Need::Done), count(Need::Place) + count(Need::Adjust), count(Need::Break) + count(Need::Drain), count(Need::Unknown), count(Need::Kept));
        s.unsupported = self.cells.iter().zip(needs).filter(|(_, n)| **n == Need::Unsupported).map(|(c, _)| (c.pos, c.target.name.clone())).collect();
        s.failed = self.tries.iter().filter_map(|(p, t)| t.why().map(|w| (*p, w))).collect();
        s.scaffold = self.scaffold.iter().filter(|p| look.solid(**p) == Some(true)).count();
        s.seals = self.seals.iter().copied().collect();
        (s.phase, s.missing) = (phase, missing);
    }

    pub(super) fn bump(&self, f: impl FnOnce(&mut BuildStatus)) {
        if let Ok(mut s) = self.status.lock() {
            f(&mut s);
        }
    }
}
