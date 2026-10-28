//! Processes: goals that compete for the bot's movement (Baritone's `PathingControlManager`). Each tick the
//! highest-priority process that wants control gets it, and one shared [`Pilot`](super::Pilot) drives towards
//! its goal. Network-free core: [`Arbiter`]; on a server: [`Processes`]. Design: docs/process-layer.md.

mod act;
mod arbiter;
mod backfill;
mod evade;
mod follow;
mod goto;
pub(crate) mod run;

use acacia_physics::{BlockPos, Vec3};

use super::hazard::Hazards;
use super::{GotoOpts, Goal, PathNode};
use crate::interact::Face;
use crate::ActionError;

pub use arbiter::{Arbiter, Decision};
pub use backfill::{Backfill, substitutes};
pub use evade::Evade;
pub use follow::{Follow, Target};
pub use goto::Goto;
pub use run::{Processes, Status};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ProcId(pub(crate) u32);

/// What a process wants this tick.
#[derive(Debug, Clone, PartialEq)]
pub enum Want {
    /// Drive towards the goal (with [`Process::opts`]).
    Go(Goal),
    /// Keep control and stand still.
    Hold,
    /// Keep control and do something in place.
    Act(Act),
    /// Let lower-priority processes have control.
    Yield,
    /// Finished: removed, with its outcome kept.
    Done(Result<(), ActionError>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Act {
    /// Place a block into `pos` (an air or liquid cell): the first of `blocks` (item identifiers) the bot holds.
    Fill { pos: BlockPos, blocks: Vec<String> },
    /// Place `item` against `face` of `against`, clicked at `click` (relative to its minimum corner).
    Place { item: String, against: BlockPos, face: Face, click: Vec3 },
    /// Break the block at `pos` with the best tool held.
    Break { pos: BlockPos },
    /// Open the container at `station` and take stacks of `items` (identifier, count wanted) while there is room.
    Restock { station: BlockPos, items: Vec<(String, u32)> },
}

/// World changes every process hears about, whichever made them.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// A path's work broke the block `block` (identifier) at `pos`.
    Dug { pos: BlockPos, block: String },
    /// A path's work placed a scaffold block at `pos` (bridging, pillaring).
    Placed { pos: BlockPos },
}

/// A block as a process sees it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlockView {
    pub name: &'static str,
    /// Registry form: `key=value`, sorted, comma separated.
    pub props: &'static str,
    pub air: bool,
    pub liquid: bool,
    /// Its shape fills the cell: any point of any face can be clicked.
    pub full: bool,
    /// A click uses it (chest, door...) instead of placing against it.
    pub interactive: bool,
}

impl BlockView {
    pub fn of(s: &acacia_world::BlockState) -> Self {
        Self { name: s.name, props: s.properties, air: s.is_air(), liquid: s.is_liquid(), full: s.is_full_cube(), interactive: crate::interact::is_interactive(s) }
    }

    pub fn solid(&self) -> bool {
        !self.air && !self.liquid
    }

    pub fn prop(&self, key: &str) -> Option<&'static str> {
        self.props.split(',').find_map(|kv| kv.split_once('=').filter(|(k, _)| *k == key).map(|(_, v)| v))
    }
}

/// What a process may look at besides the bot itself.
pub trait Look {
    /// The cell blocks movement; `None`: not loaded.
    fn solid(&self, pos: BlockPos) -> Option<bool>;
    /// Feet position of a tracked entity.
    fn find(&self, target: &Target) -> Option<Vec3>;
    /// The block at `pos`; `None`: not loaded.
    fn block(&self, _pos: BlockPos) -> Option<BlockView> {
        None
    }
    /// Items of `item` (identifier) in the inventory.
    fn count(&self, _item: &str) -> u32 {
        0
    }
    /// Empty main inventory slots.
    fn free_slots(&self) -> u32 {
        0
    }
    /// The held item's identifier.
    fn held(&self) -> Option<&str> {
        None
    }
    /// A placement into `pos` is sent and not yet confirmed (the cell shows a prediction).
    fn in_flight(&self, _pos: BlockPos) -> bool {
        false
    }
    /// Any placement is sent and not yet confirmed.
    fn any_in_flight(&self) -> bool {
        false
    }
}

pub struct Ctx<'a> {
    pub tick: u64,
    /// Feet position.
    pub pos: Vec3,
    pub on_ground: bool,
    /// The driven path's nodes still ahead (empty when nothing is driven).
    pub path: &'a [PathNode],
    /// Ticks since a process other than the asking one last had control.
    pub quiet: u64,
    pub look: &'a dyn Look,
    /// The entities as the pilot last perceived them.
    pub hazards: Option<&'a Hazards>,
}

impl Ctx<'_> {
    pub fn eye(&self) -> Vec3 {
        [self.pos[0], self.pos[1] + crate::movement::EYE_HEIGHT, self.pos[2]]
    }
}

/// A goal competing for the bot's movement. Hooks other than [`Process::want`] are called on the process
/// they concern only.
pub trait Process: Send {
    fn name(&self) -> &str;

    fn want(&mut self, cx: &Ctx) -> Want;

    /// Settings for driving to a [`Want::Go`] goal; read when the goal changes.
    fn opts(&self) -> GotoOpts {
        GotoOpts::default()
    }

    /// The pilot reached this process's goal; true: finished (`Ok`).
    fn arrived(&mut self) -> bool {
        false
    }

    /// Driving to this process's goal failed (the pilot gave up); true: finished with this error.
    fn failed(&mut self, _e: &ActionError) -> bool {
        false
    }

    /// A [`Want::Act`] (or [`Process::en_route`] placement) was done (or not).
    fn acted(&mut self, _act: &Act, _result: &Result<(), ActionError>) {}

    /// While its [`Want::Go`] drives on, an [`Act::Place`] to make without stopping (`process::en_route`).
    /// Asked after `want` on driving ticks; taking it is up to the driver.
    fn en_route(&mut self) -> Option<Act> {
        None
    }

    fn observe(&mut self, _event: &Event) {}

    fn lost_control(&mut self) {}

    fn regained(&mut self) {}
}
