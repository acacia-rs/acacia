//! Server-confirmed world actions: a break, place or door click is done only once the server's world
//! (the bot's tracked blocks, which change on `UpdateBlock` alone) shows the expected cell. On timeout the
//! cell is re-read: done, unchanged (retry) or changed otherwise (failed). Design:
//! acacia-pathfind/docs/confirmed-actions.md.

use std::time::Instant;

use acacia_client::proto::packets::{UpdateBlock, UpdateBlockSynced, UpdateSubchunkBlocks};
use acacia_client::proto::types::BlockCoordinates;
use acacia_client::proto::{Packet, RawPacket};
use acacia_physics::BlockPos;
use acacia_world::BlockState;

use super::wire::block_coordinates;
use crate::cancel::{Claim, Held};
use crate::state::ItemStack;
use crate::{ActionError, Bot};

/// What the cell must show for the action to be done.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Expect {
    /// Broken: air or liquid.
    Gone,
    /// Placed: anything that is neither.
    Solid,
    /// A block property, like a door's `open_bit`.
    Prop(&'static str, &'static str),
    /// Placed into a cell already holding block `.0` (a half slab made double): any other solid block.
    Replaced(u32),
}

impl Expect {
    fn met(self, id: u32, s: &BlockState) -> bool {
        match self {
            Expect::Gone => s.is_air() || s.is_liquid(),
            Expect::Solid => !s.is_air() && !s.is_liquid(),
            Expect::Prop(name, value) => s.property(name) == Some(value),
            Expect::Replaced(old) => id != old && !s.is_air() && !s.is_liquid(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Verdict {
    Wait,
    Done,
    /// Unchanged past the deadline (or the server restored it): send the action again.
    Retry,
    Failed(String),
}

/// One action awaiting its echo, tick-driven.
#[derive(Debug, Clone)]
pub(crate) struct Confirm {
    expect: Expect,
    /// Runtime id of the cell when the action went out.
    before: Option<u32>,
    deadline: u64,
    tries: u32,
    max_tries: u32,
    /// The bot wrote the expected block itself (a predicted placement): only the server's echo counts.
    predicted: bool,
    echo_seen: bool,
}

impl Confirm {
    /// `before`: the cell's runtime id as sent; `timeout`: ticks to wait for each try.
    pub(crate) fn new(expect: Expect, before: Option<u32>, now: u64, timeout: u64, max_tries: u32) -> Self {
        Self { expect, before, deadline: now + timeout, tries: 1, max_tries, predicted: false, echo_seen: false }
    }

    /// The cell shows the bot's own prediction until the server's echo replaces it.
    pub(crate) fn predicted(self) -> Self {
        Self { predicted: true, ..self }
    }

    pub(crate) fn tries(&self) -> u32 {
        self.tries
    }

    /// No echo arrived since the last (re)send: the cell still holds the bot's prediction, if any.
    pub(crate) fn unechoed(&self) -> bool {
        !self.echo_seen
    }

    /// The cell as of tick `now`; `echoed`: an `UpdateBlock` for it arrived since the last check.
    pub(crate) fn check(&mut self, now: u64, cell: Option<(u32, &BlockState)>, echoed: bool) -> Verdict {
        self.echo_seen |= echoed;
        let Some((id, state)) = cell else { return Verdict::Failed("the cell is no longer loaded".into()) };
        let ours = self.predicted && !self.echo_seen;
        if self.expect.met(id, state) && !ours {
            return Verdict::Done;
        }
        let unchanged = ours || self.before.is_none_or(|b| b == id);
        // An echo showing the old block is the server refusing: no need to wait out the deadline.
        if now < self.deadline && !(echoed && unchanged) {
            return Verdict::Wait;
        }
        match (unchanged, self.tries < self.max_tries) {
            (true, true) => Verdict::Retry,
            (true, false) => Verdict::Failed(format!("the server showed no change after {} tries", self.tries)),
            (false, _) => Verdict::Failed(format!("the cell became {}", state.name)),
        }
    }

    /// The action went out again at `now`.
    pub(crate) fn retried(&mut self, now: u64, timeout: u64) {
        self.tries += 1;
        self.deadline = now + timeout;
        self.echo_seen = false;
    }
}

/// The stack to put back after a refused placement: ours, unless the server has set the slot since.
pub(crate) fn unpredict(now: &ItemStack, predicted: &ItemStack, before: &ItemStack) -> Option<ItemStack> {
    (now == predicted && predicted != before).then(|| before.clone())
}

/// The slot with a failed placement's item given back, whatever later predictions did to it since: with several
/// placements from one stack in flight, [`unpredict`]'s exact match never held, the count fell behind the
/// server's, and a slot read empty that the server still filled (live: moves into it refused, status 50).
/// `None`: the slot now holds something else; the server's own update decides it.
pub(crate) fn give_back(now: &ItemStack, predicted: &ItemStack, before: &ItemStack) -> Option<ItemStack> {
    let consumed = before.count.checked_sub(if predicted.is_empty() { 0 } else { predicted.count }).filter(|c| *c > 0)?;
    if now.is_empty() {
        return Some(ItemStack { count: consumed, ..before.clone() });
    }
    (now.network_id == before.network_id && now.stack_network_id == before.stack_network_id).then(|| ItemStack { count: now.count + consumed, ..now.clone() })
}

/// A server block update for layer 0 of `at`, in any of the packets the world tracker applies.
fn is_update_at(p: &RawPacket, at: &BlockCoordinates) -> bool {
    match p.id {
        UpdateBlock::ID => p.decode::<UpdateBlock>().is_ok_and(|u| u.position == *at && u.layer == 0),
        UpdateBlockSynced::ID => p.decode::<UpdateBlockSynced>().is_ok_and(|u| u.position == *at && u.layer == 0),
        UpdateSubchunkBlocks::ID => p.decode::<UpdateSubchunkBlocks>().is_ok_and(|u| u.blocks.iter().any(|b| b.position == *at)),
        _ => false,
    }
}

impl Bot {
    /// Writes block `wire` (the server's id form) into the tracked world at `pos` ahead of the server, as the
    /// vanilla client does when it places: physics then agrees with a server that already has it. Returns the
    /// block it replaced (wire id), for [`Bot::unpredict_block`].
    pub(crate) fn predict_block(&self, pos: BlockPos, wire: u32) -> Option<u32> {
        let view = self.world.as_ref()?.view()?;
        let (id, _) = self.block_at(pos)?;
        let old = view.world().wire_id(id);
        view.set_block(pos[0], pos[1], pos[2], 0, wire);
        Some(old)
    }

    /// Puts back the block a refused or unanswered prediction replaced (no ghost block stays behind).
    pub(crate) fn unpredict_block(&self, pos: BlockPos, old: u32) {
        if let Some(view) = self.world.as_ref().and_then(|w| w.view()) {
            view.set_block(pos[0], pos[1], pos[2], 0, old);
        }
    }

    /// Starts confirming an action on `pos` sent this tick.
    pub(crate) fn confirmation(&self, pos: BlockPos, expect: Expect, max_tries: u32) -> Confirm {
        let before = self.block_at(pos).map(|(id, _)| id);
        Confirm::new(expect, before, self.governor.now(), self.governor.timeout_ticks(), max_tries)
    }

    /// Lets ticks pass until `c` decides. A first try's echo feeds the governor's round-trip estimate.
    pub(crate) async fn await_confirm(&mut self, pos: BlockPos, c: &mut Confirm, sent_at: Instant) -> Result<Verdict, ActionError> {
        let at = block_coordinates(pos);
        let t = Instant::now();
        loop {
            let echoed = self.next_tick(|_, p| is_update_at(p, &at)).await?;
            let verdict = c.check(self.governor.now(), self.block_at(pos), echoed);
            if verdict == Verdict::Done && c.tries() == 1 && echoed {
                self.governor.sample(sent_at.elapsed());
            }
            if verdict != Verdict::Wait {
                self.act_timing.add(crate::act_timing::Phase::Echo, t.elapsed());
                if verdict == Verdict::Retry {
                    self.act_timing.retry();
                }
                let held = self.state.items.name(self.state.inventory.held().network_id).unwrap_or("-");
                let cell = self.block_at(pos).map_or("unloaded", |(_, s)| s.name);
                tracing::debug!(?pos, ?verdict, tries = c.tries(), echoed, ms = t.elapsed().as_millis() as u64, held, cell, "action confirmation");
                return Ok(verdict);
            }
        }
    }

    /// Lets ticks pass until the governor allows another action, then counts it as sent; [`Bot::release`] settles it.
    pub(crate) async fn admit_action(&mut self) -> Result<Claim, ActionError> {
        let t = Instant::now();
        while !self.governor.may_send() {
            self.next_tick(|_, _| false).await?;
        }
        self.act_timing.add(crate::act_timing::Phase::Admit, t.elapsed());
        self.governor.sent(false);
        Ok(self.claim(Held::Slot))
    }

    /// The governor every world action asks before going out.
    pub fn governor(&self) -> &crate::governor::Governor {
        &self.governor
    }

    pub fn governor_mut(&mut self) -> &mut crate::governor::Governor {
        &mut self.governor
    }
}

#[cfg(test)]
#[path = "confirm_tests.rs"]
mod tests;
