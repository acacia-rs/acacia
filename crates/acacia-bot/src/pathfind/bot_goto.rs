//! The pathfinding methods on [`Bot`].

use acacia_physics::BlockPos;

use super::goal::Goal;
use super::goto::GotoOpts;
use super::process::{Goto, Processes};
use crate::{ActionError, Bot};

impl Bot {
    /// Walks to `goal` (physics bots only). See [`Navigator`](super::Navigator) for step-wise control.
    pub async fn goto(&mut self, goal: Goal) -> Result<(), ActionError> {
        self.goto_with(goal, GotoOpts::default()).await
    }

    /// [`Bot::run`] with a single [`Goto`].
    pub async fn goto_with(&mut self, goal: Goal, opts: GotoOpts) -> Result<(), ActionError> {
        let mut procs = Processes::new();
        let id = procs.add(Goto::with(goal, opts), 0);
        self.run(&mut procs).await?;
        procs.take_outcome(id).unwrap_or(Ok(()))
    }

    /// Releases the movement controls, e.g. after dropping a `goto` future or a [`Navigator`].
    pub fn stop_pathing(&mut self) {
        if let Some(c) = self.controls() {
            c.stop();
        }
    }

    /// Feet block position of a physics bot (for building goals).
    pub fn block_position(&self) -> Option<BlockPos> {
        let p = self.backend.movement()?.position()?;
        Some([p[0].floor() as i32, (p[1] + 0.01).floor() as i32, p[2].floor() as i32])
    }
}
