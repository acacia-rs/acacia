//! Auto-eat's move of food from the main inventory into the hotbar, run from the client tick like
//! the rest of auto-eat (an awaiting action would break if the caller dropped `Bot::next`): open
//! the own inventory (Geyser ignores requests without a screen), wait a click, send one
//! `ItemStackRequest` (`Place` into an empty hotbar slot, else `Swap` with the selected one), wait
//! for its response, wait a click and close.

use acacia_client::proto::RawPacket;

use super::equip::hotbar_target;
use crate::human::{CLICK, HOTBAR_SWITCH};
use crate::items::{response_for, Op, Plan, SlotRef};
use crate::state::GameState;
use crate::Bot;

/// Ticks to wait for the server to confirm the inventory opened; BDS may never answer.
const OPEN_TICKS: u32 = 20;
const RESPONSE_TICKS: u32 = 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Opening { left: u32 },
    Clicking { left: u32 },
    Waiting { left: u32 },
    Closing { left: u32, ok: bool },
}

/// What the fetch sees this tick.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct FetchCtx {
    /// The server confirmed the inventory screen.
    pub opened: bool,
    /// The request's response arrived: accepted or not.
    pub response: Option<bool>,
    /// An action started awaiting, or the player died.
    pub interrupted: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FetchStep {
    Nothing,
    Send,
    /// Close the screen; the fetch is over with this outcome.
    Close(bool),
}

#[derive(Debug)]
pub(crate) struct Fetch {
    phase: Phase,
    pending: Option<(Plan, i32)>,
}

impl Fetch {
    pub fn new() -> Self {
        Self { phase: Phase::Opening { left: OPEN_TICKS }, pending: None }
    }

    pub fn tick(&mut self, ctx: &FetchCtx, mut click: impl FnMut() -> u32) -> FetchStep {
        use Phase as P;
        let (phase, step) = match self.phase {
            P::Opening { .. } | P::Clicking { .. } if ctx.interrupted => (P::Closing { left: 0, ok: false }, FetchStep::Close(false)),
            P::Opening { left } if ctx.opened || left == 0 => (P::Clicking { left: click() }, FetchStep::Nothing),
            P::Opening { left } => (P::Opening { left: left - 1 }, FetchStep::Nothing),
            P::Clicking { left: 0 } => (P::Waiting { left: RESPONSE_TICKS }, FetchStep::Send),
            P::Clicking { left } => (P::Clicking { left: left - 1 }, FetchStep::Nothing),
            P::Waiting { .. } if ctx.response.is_some() => {
                (P::Closing { left: click(), ok: ctx.response == Some(true) }, FetchStep::Nothing)
            }
            P::Waiting { left: 0 } => (P::Closing { left: 0, ok: false }, FetchStep::Nothing),
            P::Waiting { left } => (P::Waiting { left: left - 1 }, FetchStep::Nothing),
            P::Closing { left: 0, ok } => (self.phase, FetchStep::Close(ok)),
            P::Closing { left, ok } => (P::Closing { left: left - 1, ok }, FetchStep::Nothing),
        };
        self.phase = phase;
        step
    }

    /// The request could not be built or sent.
    pub fn send_failed(&mut self) {
        self.phase = Phase::Closing { left: 0, ok: false };
    }
}

/// The op moving the whole stack in main slot `from` to the hotbar.
pub(crate) fn fetch_op(state: &GameState, from: u8) -> Op {
    let to = hotbar_target(&state.inventory);
    let (from, to) = (SlotRef::Main(from), SlotRef::Main(to));
    match (from.stack(state), to.stack(state)) {
        (Some(stack), Some(dst)) if dst.is_empty() => Op::Transfer { from, to, count: u8::try_from(stack.count).unwrap_or(u8::MAX) },
        _ => Op::Swap { a: from, b: to },
    }
}

impl Bot {
    pub(crate) fn start_fetch(&mut self, from: u8) {
        self.survival.fetch = Some((Fetch::new(), from));
        self.send_open_inventory();
    }

    pub(crate) fn tick_fetch(&mut self) {
        let Some((fetch, from)) = self.survival.fetch.as_mut() else { return };
        let ctx = FetchCtx {
            opened: self.state.containers.open.is_some(),
            response: self.survival.fetch_response.take(),
            interrupted: !self.survival.idle || !self.state.player.alive,
        };
        let human = &mut self.human;
        let from = *from;
        match fetch.tick(&ctx, || human.ticks_between(CLICK)) {
            FetchStep::Nothing => {}
            FetchStep::Send => match self.send_ops(&[fetch_op(&self.state, from)]) {
                Ok(pending) => self.fetch_mut().pending = Some(pending),
                Err(e) => {
                    tracing::debug!(error = %e, "auto-eat could not move food to the hotbar");
                    self.fetch_mut().send_failed();
                }
            },
            FetchStep::Close(ok) => {
                self.survival.fetch = None;
                self.close_own_inventory_screen();
                let after = self.human.ticks_between(HOTBAR_SWITCH);
                self.survival.auto_eat.fetched(ok, after);
            }
        }
    }

    /// Settles the fetch's request when `packet` carries its response.
    pub(crate) fn on_fetch_packet(&mut self, packet: &RawPacket) {
        let Some((Fetch { pending: Some((_, id)), .. }, _)) = &self.survival.fetch else { return };
        let Some(response) = response_for(packet, *id) else { return };
        let Some((plan, _)) = self.fetch_mut().pending.take() else { return };
        let ok = self.finish_request(plan, &response).is_ok();
        self.survival.fetch_response = Some(ok);
    }

    fn fetch_mut(&mut self) -> &mut Fetch {
        &mut self.survival.fetch.as_mut().expect("a fetch is running").0
    }
}
