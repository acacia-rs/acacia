//! Placements sent without awaiting their echo: each is predicted locally, then decided from echoes on later
//! ticks while the bot goes on; the governor caps how many are in flight. A refused or unanswered one is undone
//! (block and stack) and reported failed, not retried: its owner places it again if it still wants it.
//! Network-free core: [`Pipeline`]; on a server: [`Bot::place_sent`], [`Bot::settle_placements`].
//! Design: acacia-pathfind/docs/confirmed-actions.md "Pipelined placements".

use std::time::Instant;

use acacia_client::proto::packets::{UpdateBlock, UpdateBlockSynced, UpdateSubchunkBlocks};
use acacia_client::proto::{Packet, RawPacket};
use acacia_physics::{BlockPos, Vec3};
use acacia_world::BlockState;

use super::confirm::{give_back, Confirm, Expect, Verdict};
use super::wire::SwingSource;
use super::Face;
use crate::act_timing::Phase;
use crate::state::ItemStack;
use crate::{ActionError, Bot};

/// What undoing a refused placement takes, kept by whoever sent it.
#[derive(Debug, Clone)]
pub(crate) struct Undo<T> {
    /// The block the prediction replaced (wire id), if one was written.
    pub replaced: Option<u32>,
    pub extra: T,
}

struct Item<T> {
    target: BlockPos,
    confirm: Confirm,
    echoed: bool,
    undo: Undo<T>,
    sent_at: Instant,
}

/// A decided placement: `undo` is `Some` when it failed and the prediction must be taken back.
pub(crate) struct Decided<T> {
    pub target: BlockPos,
    pub result: Result<(), String>,
    /// Decided on its first try by an echo (a clean round-trip sample).
    pub echoed: bool,
    pub waited: std::time::Duration,
    pub undo: Option<Undo<T>>,
}

/// Placements in flight, network-free.
pub(crate) struct Pipeline<T> {
    items: Vec<Item<T>>,
}

impl<T> Default for Pipeline<T> {
    fn default() -> Self {
        Self { items: Vec::new() }
    }
}

impl<T> Pipeline<T> {
    pub fn push(&mut self, target: BlockPos, confirm: Confirm, undo: Undo<T>) {
        self.items.push(Item { target, confirm, echoed: false, undo, sent_at: Instant::now() });
    }

    pub fn targets(&self) -> impl Iterator<Item = BlockPos> + '_ {
        self.items.iter().map(|i| i.target)
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Drops every placement; returns how many there were.
    pub fn clear(&mut self) -> usize {
        std::mem::take(&mut self.items).len()
    }

    /// A block update for `pos` arrived.
    pub fn echo(&mut self, pos: BlockPos) {
        self.items.iter_mut().filter(|i| i.target == pos).for_each(|i| i.echoed = true);
    }

    /// Decides what can be decided at tick `now`, reading cells through `cell`.
    pub fn settle<'s>(&mut self, now: u64, cell: impl Fn(BlockPos) -> Option<(u32, &'s BlockState)>) -> Vec<Decided<T>> {
        let mut out = Vec::new();
        let mut i = 0;
        while i < self.items.len() {
            let it = &mut self.items[i];
            let echoed = std::mem::take(&mut it.echoed);
            let verdict = it.confirm.check(now, cell(it.target), echoed);
            if verdict == Verdict::Wait {
                i += 1;
                continue;
            }
            let it = self.items.swap_remove(i);
            let (first, waited) = (it.confirm.tries() == 1 && echoed, it.sent_at.elapsed());
            out.push(match verdict {
                Verdict::Done => Decided { target: it.target, result: Ok(()), echoed: first, waited, undo: None },
                v => {
                    let why = if let Verdict::Failed(why) = v { why } else { "the server did not place it".into() };
                    let replaced = it.undo.replaced.filter(|_| it.confirm.unechoed());
                    Decided { target: it.target, result: Err(why), echoed: first, waited, undo: Some(Undo { replaced, extra: it.undo.extra }) }
                }
            });
        }
        out
    }
}

/// The stack change a sent placement predicted, to put back if it fails.
#[derive(Debug, Clone)]
pub(crate) struct StackUndo {
    slot: u8,
    before: ItemStack,
    predicted: ItemStack,
}

impl Bot {
    /// [`Bot::place_block_at`] that returns once the click is out; [`Bot::settle_placements`] tells how it went.
    pub(crate) async fn place_sent(&mut self, against: BlockPos, face: Face, click: Vec3) -> Result<(), ActionError> {
        let (target, half_slab) = self.aim_placement(against, face, click).await?;
        if self.in_flight.targets().any(|t| t == target) {
            return Err(ActionError::NotPossible(format!("a placement at {target:?} is already in flight")));
        }
        let admitted = self.admit_action().await?;
        let (slot, before) = (self.state.inventory.selected_hotbar_slot, self.state.inventory.held().clone());
        let expect = half_slab.map_or(Expect::Solid, Expect::Replaced);
        let mut confirm = self.confirmation(target, expect, 1);
        if half_slab.is_none() {
            confirm = confirm.predicted();
        }
        self.click_block(against, face, click, SwingSource::Build)?;
        let speed = self.movement.as_ref().and_then(|m| m.state()).map_or(0.0, |s| s.vel[0].hypot(s.vel[2]));
        let (dist, support) = (super::block_distance(self.eye_position(), against), self.block_at(against).map_or("?", |(_, s)| s.name));
        tracing::debug!(?target, nth = self.governor.sent_this_tick(), speed, dist, ?face, support, "pipelined placement sent");
        let replaced = (before.block_runtime_id != 0 && half_slab.is_none()).then(|| self.predict_block(target, before.block_runtime_id)).flatten();
        let predicted = self.state.inventory.held().clone();
        self.in_flight.push(target, confirm, Undo { replaced, extra: StackUndo { slot, before, predicted } });
        self.unclaim(admitted);
        Ok(())
    }

    /// Gives up on placements nobody will settle (their processes run was abandoned): decides those that can be
    /// decided now and frees the governor slots of the rest, whose echoes then correct the world on their own.
    pub(crate) fn forget_placements(&mut self) {
        self.settle_placements();
        for _ in 0..self.in_flight.clear() {
            self.governor.settled();
        }
    }

    /// Placements sent and not decided yet.
    pub(crate) fn placements_in_flight(&self) -> impl Iterator<Item = BlockPos> + '_ {
        self.in_flight.targets()
    }

    /// Notes a block update for any in-flight target (every packet an action sees passes here).
    pub(crate) fn note_echo(&mut self, p: &RawPacket) {
        if self.in_flight.is_empty() {
            return;
        }
        let positions: Vec<BlockPos> = match p.id {
            UpdateBlock::ID => p.decode::<UpdateBlock>().ok().filter(|u| u.layer == 0).map(|u| vec![[u.position.x, u.position.y, u.position.z]]).unwrap_or_default(),
            UpdateBlockSynced::ID => p.decode::<UpdateBlockSynced>().ok().filter(|u| u.layer == 0).map(|u| vec![[u.position.x, u.position.y, u.position.z]]).unwrap_or_default(),
            UpdateSubchunkBlocks::ID => p.decode::<UpdateSubchunkBlocks>().map(|u| u.blocks.iter().map(|b| [b.position.x, b.position.y, b.position.z]).collect()).unwrap_or_default(),
            _ => return,
        };
        positions.into_iter().for_each(|pos| self.in_flight.echo(pos));
    }

    /// Decides the in-flight placements that can be decided now, undoing failed ones; returns each one's target
    /// and outcome.
    pub(crate) fn settle_placements(&mut self) -> Vec<(BlockPos, Result<(), ActionError>)> {
        let now = self.governor.now();
        let mut pipeline = std::mem::take(&mut self.in_flight);
        let decided = pipeline.settle(now, |p| self.block_at(p));
        self.in_flight = pipeline;
        decided
            .into_iter()
            .map(|d| {
                self.governor.settled();
                self.act_timing.add(Phase::Echo, d.waited);
                if d.echoed && d.result.is_ok() {
                    self.governor.sample(d.waited);
                }
                if let Some(undo) = d.undo {
                    let s = &undo.extra;
                    self.act_timing.retry();
                    if let Some(old) = undo.replaced {
                        self.unpredict_block(d.target, old);
                    }
                    let main = &mut self.state.inventory.main[usize::from(s.slot)];
                    if let Some(back) = give_back(main, &s.predicted, &s.before) {
                        *main = back;
                    }
                }
                let target = d.target;
                tracing::debug!(?target, result = ?d.result, "pipelined placement decided");
                (target, d.result.map_err(|why| ActionError::Rejected(format!("placing at {target:?}: {why}"))))
            })
            .collect()
    }
}
