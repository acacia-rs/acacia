//! A builder's run journal: one JSON line per tick (what it wanted and why) plus its acts, walks and rests,
//! flushed as written so a killed run still leaves the file. `build::findings` reads the journals of every bot on
//! a job afterwards. The same lines come out of the simulations (time = tick × 50 ms there).

use std::io::{BufWriter, Write};
use std::path::Path;
use std::sync::Mutex;
use std::time::Instant;

use acacia_physics::{BlockPos, Vec3};
use serde::{Deserialize, Serialize};

use super::status::{HoldWhy, Phase};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "e", rename_all = "snake_case")]
pub enum Entry {
    /// What the builder asked for this tick.
    Tick {
        feet: Vec3,
        want: Want,
        phase: Phase,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        why: Option<HoldWhy>,
        /// The cell acted on or walked for.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cell: Option<BlockPos>,
        /// What this bot's world shows in `cell` (`name[props]`; none: not loaded).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        saw: Option<String>,
        claimed: usize,
        taken: usize,
        to_place: usize,
    },
    /// An act's outcome, as the process layer reported it.
    Acted {
        kind: Kind,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cell: Option<BlockPos>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        item: Option<String>,
        /// The structure's block for `cell` (none: outside it, such as scaffold).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        target: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        err: Option<String>,
    },
    WalkStart { task: BlockPos, stands: Vec<BlockPos> },
    /// `acts`: acts done while the walk was under way (clicks in reach before arriving).
    WalkEnd {
        task: BlockPos,
        end: WalkEnd,
        ticks: u64,
        ms: u64,
        dist: f32,
        #[serde(default)]
        acts: u32,
        /// Why the pilot gave up (failed walks).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        err: Option<String>,
    },
    /// A cell set aside for a while (no stand, no click from the stand, failed acts).
    Rest { cell: BlockPos },
    /// A placement handed over to be made while walking on (its outcome comes as an `Acted` later).
    EnRoute { cell: BlockPos },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Want {
    Act,
    Go,
    Hold,
    Done,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Place,
    Break,
    Fill,
    Restock,
    /// A use-click (`Act::Use`).
    Use,
    /// Sign text written.
    Sign,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WalkEnd {
    Arrived,
    Failed,
    /// Replaced by a goal for another task before arriving.
    Switched,
    /// Another process took the controls.
    Lost,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Line {
    /// Milliseconds since the journal opened (simulations: tick × 50).
    pub t: u64,
    pub tick: u64,
    pub bot: String,
    #[serde(flatten)]
    pub e: Entry,
}

struct Walk {
    task: BlockPos,
    tick: u64,
    t: u64,
    from: Vec3,
    acts: u32,
}

struct Inner {
    out: BufWriter<Box<dyn Write + Send>>,
    walk: Option<Walk>,
    /// Feet at the last tick line (walk ends are reported between ticks).
    feet: Vec3,
}

pub struct Journal {
    bot: String,
    /// None: simulated time.
    t0: Option<Instant>,
    /// Simulated time: the world's tick, when the builder's own (decisions only) would skip ticks spent acting.
    clock: Option<std::sync::Arc<std::sync::atomic::AtomicU64>>,
    inner: Mutex<Inner>,
}

impl Journal {
    /// A journal for `bot` written to `out`; `wall`: timestamps from the clock, else from the tick.
    pub fn new(bot: impl Into<String>, out: Box<dyn Write + Send>, wall: bool) -> Self {
        let inner = Mutex::new(Inner { out: BufWriter::new(out), walk: None, feet: [0.0; 3] });
        Self { bot: bot.into(), t0: wall.then(Instant::now), clock: None, inner }
    }

    /// Timestamps from `clock` (ticks) instead.
    pub fn clocked(mut self, clock: std::sync::Arc<std::sync::atomic::AtomicU64>) -> Self {
        self.clock = Some(clock);
        self
    }

    /// `dir/<bot>.jsonl`, wall-clock timestamps.
    pub fn create(dir: &Path, bot: &str) -> std::io::Result<Self> {
        std::fs::create_dir_all(dir)?;
        Ok(Self::new(bot, Box::new(std::fs::File::create(dir.join(format!("{bot}.jsonl")))?), true))
    }

    fn t(&self, tick: u64) -> u64 {
        match (&self.t0, &self.clock) {
            (Some(t0), _) => t0.elapsed().as_millis() as u64,
            (None, Some(c)) => c.load(std::sync::atomic::Ordering::Relaxed) * 50,
            (None, None) => tick * 50,
        }
    }

    pub fn write(&self, tick: u64, e: Entry) {
        let line = Line { t: self.t(tick), tick, bot: self.bot.clone(), e };
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        match &line.e {
            Entry::Tick { feet, .. } => inner.feet = *feet,
            Entry::Acted { .. } => inner.walk.iter_mut().for_each(|w| w.acts += 1),
            _ => {}
        }
        if let Ok(s) = serde_json::to_string(&line) {
            let _ = writeln!(inner.out, "{s}").and_then(|_| inner.out.flush());
        }
    }

    /// Ends any walk under way (as `Switched`), then starts one for `task`.
    pub fn walk_start(&self, tick: u64, feet: Vec3, task: BlockPos, stands: Vec<BlockPos>) {
        self.walk_end(tick, WalkEnd::Switched);
        self.inner.lock().unwrap_or_else(|e| e.into_inner()).walk = Some(Walk { task, tick, t: self.t(tick), from: feet, acts: 0 });
        self.write(tick, Entry::WalkStart { task, stands });
    }

    /// Ends the walk under way, if any.
    pub fn walk_end(&self, tick: u64, end: WalkEnd) {
        self.walk_failed(tick, end, None);
    }

    /// Ends the walk under way, if any, with the pilot's reason for giving up.
    pub fn walk_failed(&self, tick: u64, end: WalkEnd, err: Option<String>) {
        let (walk, feet) = {
            let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            (inner.walk.take(), inner.feet)
        };
        let Some(w) = walk else { return };
        let d = [feet[0] - w.from[0], feet[1] - w.from[1], feet[2] - w.from[2]];
        let dist = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
        self.write(tick, Entry::WalkEnd { task: w.task, end, ticks: tick - w.tick, ms: self.t(tick).saturating_sub(w.t), dist, acts: w.acts, err });
    }
}

impl super::Builder {
    /// Writes this builder's journal to `journal`.
    pub fn with_journal(mut self, journal: std::sync::Arc<Journal>) -> Self {
        self.journal = Some(journal);
        self
    }

    pub(super) fn note(&self, e: Entry) {
        if let Some(j) = &self.journal {
            j.write(self.tick, e);
        }
    }
}

/// Every line of the `*.jsonl` journals in `dir`, in time order.
pub fn read_dir(dir: &Path) -> std::io::Result<Vec<Line>> {
    let mut lines = Vec::new();
    for f in std::fs::read_dir(dir)? {
        let path = f?.path();
        if path.extension().is_some_and(|x| x == "jsonl") {
            lines.extend(std::fs::read_to_string(&path)?.lines().filter_map(|l| serde_json::from_str::<Line>(l).ok()));
        }
    }
    lines.sort_by_key(|l| (l.t, l.tick));
    Ok(lines)
}
