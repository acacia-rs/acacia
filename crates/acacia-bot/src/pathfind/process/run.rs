//! [`Arbiter`] on a server: the bot's world and entities to look at, its actions to drive and act with.

use acacia_physics::{BlockPos, Vec3};

use super::arbiter::{Arbiter, Decision};
use super::act;
use super::{BlockView, Look, ProcId, Process, Target};
use crate::act_timing::Phase;
use crate::pathfind::goto::step;
use crate::pathfind::{MovementProfile, NavStatus, Pilot};
use crate::{ActionError, Bot};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// Driving to the goal of the process in control.
    Moving(NavStatus),
    /// Standing still for the process in control.
    Holding,
    /// The process in control placed or broke something.
    Acted,
    /// Every process yields.
    Idle,
    /// No processes left.
    Empty,
}

/// Processes run by one bot: add them with a priority (higher wins), then call [`Processes::tick`] once
/// per tick or [`Bot::run`] until every process has finished.
#[derive(Default)]
pub struct Processes {
    arbiter: Arbiter,
    /// Placements sent without awaiting their echo, by target: whose they are, to tell it how they went.
    sent: std::collections::HashMap<BlockPos, (ProcId, super::Act)>,
    walk: en_route::Walk,
}

mod en_route;

impl Processes {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, proc: impl Process + 'static, priority: i32) -> ProcId {
        self.arbiter.add(proc, priority)
    }

    pub fn pause(&mut self, id: ProcId) -> bool {
        self.arbiter.pause(id)
    }

    pub fn resume(&mut self, id: ProcId) -> bool {
        self.arbiter.resume(id)
    }

    pub fn remove(&mut self, id: ProcId) -> Option<Box<dyn Process>> {
        self.arbiter.remove(id)
    }

    pub fn arbiter(&self) -> &Arbiter {
        &self.arbiter
    }

    pub fn outcome(&self, id: ProcId) -> Option<&Result<(), ActionError>> {
        self.arbiter.outcome(id)
    }

    pub fn take_outcome(&mut self, id: ProcId) -> Option<Result<(), ActionError>> {
        self.arbiter.take_outcome(id)
    }

    pub fn pilot(&self) -> Option<&Pilot> {
        self.arbiter.pilot()
    }

    /// Gives control to one process for a tick and lets the tick pass (not once every process has finished).
    /// Errors only when the bot cannot move or is disconnected; processes' own failures are their outcomes.
    pub async fn tick(&mut self, bot: &mut Bot) -> Result<Status, ActionError> {
        let Some(movement) = bot.movement.as_ref() else {
            return Err(ActionError::NotPossible("processes need a physics bot (BotConfig.physics)".into()));
        };
        let Some(live) = movement.state() else {
            bot.next_tick(|_, _| false).await?;
            return Ok(Status::Idle);
        };
        let (pos, on_ground) = (live.pos, movement.on_ground());
        for (target, result) in bot.settle_placements() {
            if let Some((id, act)) = self.sent.remove(&target) {
                self.arbiter.acted(id, &act, &result);
            }
        }
        let t = std::time::Instant::now();
        let decision = self.arbiter.decide(pos, on_ground, &BotLook(bot));
        bot.act_timing.add(Phase::Plan, t.elapsed());
        let acting = matches!(decision, Decision::Act(..));
        if !matches!(decision, Decision::Go) {
            self.walk.drop_aim();
        }
        if matches!(decision, Decision::Hold | Decision::Idle | Decision::Empty) {
            bot.stop_pathing();
        }
        let status = match decision {
            Decision::Go => {
                let Some(pilot) = self.arbiter.pilot_mut() else { return Ok(Status::Idle) };
                let (result, worked) = step(pilot, bot).await;
                if result == Err(ActionError::Disconnected) {
                    return Err(ActionError::Disconnected);
                }
                let status = result.as_ref().map_or(Status::Idle, |s| Status::Moving(*s));
                if !matches!(result, Ok(NavStatus::Searching | NavStatus::Moving | NavStatus::Working)) {
                    bot.stop_pathing();
                }
                let moving = result == Ok(NavStatus::Moving);
                self.arbiter.placed(worked.placed);
                self.arbiter.drove(result, worked.dug);
                self.place_en_route(bot, moving).await?;
                status
            }
            Decision::Act(id, act) => {
                bot.stop_pathing();
                let t = std::time::Instant::now();
                let human = self.arbiter.get(id).is_some_and(|p| p.opts().search.profile == MovementProfile::Human);
                // Optimal: placements go out without awaiting their echo; their outcome comes on a later tick.
                let paced = pacing(&act, human, &bot.governor, needs_stack_request(bot, &act) && !bot.in_flight.is_empty());
                let result = match paced {
                    Pacing::Hold => None,
                    Pacing::Send => match bounded(act::send_place(bot, &act)).await {
                        Ok(target) => {
                            self.sent.insert(target, (id, act.clone()));
                            None
                        }
                        Err(e) => Some(Err(e)),
                    },
                    Pacing::Await => Some(bounded(act::perform(bot, &act, human)).await),
                };
                bot.act_timing.add(Phase::Act, t.elapsed());
                if let Some(result) = result {
                    if result == Err(ActionError::Disconnected) {
                        return Err(ActionError::Disconnected);
                    }
                    self.arbiter.acted(id, &act, &result);
                }
                if paced == Pacing::Hold { Status::Holding } else { Status::Acted }
            }
            Decision::Hold => Status::Holding,
            Decision::Idle => Status::Idle,
            Decision::Empty => Status::Empty,
        };
        if self.arbiter.is_empty() {
            bot.stop_pathing();
            return Ok(Status::Empty);
        }
        let t = std::time::Instant::now();
        bot.next_tick(|_, _| false).await?;
        if acting {
            bot.act_timing.add(Phase::PostTick, t.elapsed());
        }
        Ok(status)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Pacing {
    /// Send without awaiting the echo.
    Send,
    /// Not this tick: the in-flight cap is full (never wait for it inside the tick: in-flight placements are
    /// decided only at the start of a tick, so that wait never ends; live: the bot went silent, twice), or the
    /// item has to be moved into the hotbar while placements are in flight: BDS re-ids a stack it places from,
    /// and a move naming the old id is refused (status 50, FailedToValidateDstSlot; live: a floor log given up).
    Hold,
    /// Perform and await (Human, and everything but placements).
    Await,
}

/// Longest an act may take before it fails as timed out (a restock moves a few stacks in well under a second).
const ACT_LIMIT: std::time::Duration = std::time::Duration::from_secs(10);

/// `act` failing with [`ActionError::Timeout`] after [`ACT_LIMIT`]: an awaited reply that never came held a live
/// build for 820 s until its watchdog (2026-10-21, an act at tick 705; the journal names the next one).
async fn bounded<T>(act: impl std::future::Future<Output = Result<T, ActionError>>) -> Result<T, ActionError> {
    tokio::time::timeout(ACT_LIMIT, act).await.unwrap_or_else(|_| {
        tracing::warn!("act timed out after {ACT_LIMIT:?}");
        Err(ActionError::Timeout)
    })
}

/// The placement's item is outside the hotbar: equipping it is an item stack request.
fn needs_stack_request(bot: &Bot, act: &super::Act) -> bool {
    let super::Act::Place { item, .. } = act else { return false };
    matches!(super::act::find(bot, item), Some(crate::items::SlotRef::Main(i)) if i >= crate::state::Inventory::HOTBAR_SLOTS)
}

/// `moving`: the placement's item must be moved into the hotbar while placements are in flight.
pub(crate) fn pacing(act: &super::Act, human: bool, governor: &crate::governor::Governor, moving: bool) -> Pacing {
    match act {
        super::Act::Place { .. } if !human && (!governor.may_send() || moving) => Pacing::Hold,
        super::Act::Place { .. } if !human => Pacing::Send,
        _ => Pacing::Await,
    }
}

#[cfg(test)]
#[test]
fn a_full_in_flight_cap_holds_instead_of_waiting_inside_the_tick() {
    use crate::governor::{Governor, Limits};
    let place = super::Act::Place { item: "minecraft:dirt".into(), against: [0, 0, 0], face: crate::interact::Face::Up, click: [0.5, 1.0, 0.5] };
    let mut g = Governor::new(Limits { max_gap_ticks: 1, ..Limits::default() });
    for _ in 0..Limits::DEFAULT_IN_FLIGHT {
        assert_eq!(pacing(&place, false, &g, false), Pacing::Send);
        g.sent(false);
        g.tick(None);
    }
    assert_eq!(pacing(&place, false, &g, false), Pacing::Hold, "cap full");
    assert_eq!(pacing(&place, true, &g, false), Pacing::Await, "Human awaits each placement");
    assert_eq!(pacing(&super::Act::Break { pos: [0, 0, 0] }, false, &g, false), Pacing::Await);
    g.settled();
    assert_eq!(pacing(&place, false, &g, false), Pacing::Send, "one decided: room again");
    assert_eq!(pacing(&place, false, &g, true), Pacing::Hold, "a move into the hotbar waits for the in-flight ones");
}

impl Bot {
    /// Runs `procs` until every process has finished; their outcomes stay in `procs`.
    pub async fn run(&mut self, procs: &mut Processes) -> Result<(), ActionError> {
        while procs.tick(self).await? != Status::Empty {}
        Ok(())
    }
}

pub(crate) struct BotLook<'a>(pub &'a Bot);

impl Look for BotLook<'_> {
    fn solid(&self, pos: BlockPos) -> Option<bool> {
        self.0.block_at(pos).map(|(_, s)| !s.is_air() && !s.is_liquid())
    }

    fn find(&self, target: &Target) -> Option<Vec3> {
        let entities = &self.0.state.entities;
        let e = match target {
            Target::Player(name) => entities.players().find(|e| e.username.as_deref() == Some(name.as_str())),
            Target::Entity(id) => entities.get(*id),
        }?;
        let f = e.feet();
        Some([f.x, f.y, f.z])
    }

    fn block(&self, pos: BlockPos) -> Option<BlockView> {
        let (_, s) = self.0.block_at(pos)?;
        Some(BlockView::of(s))
    }

    fn color(&self, pos: BlockPos) -> Option<u8> {
        match self.0.state.block_entities.get(pos)?.value.get("color") {
            Some(acacia_client::proto::nbt::Value::Byte(c)) => Some(*c as u8),
            _ => None,
        }
    }

    fn count(&self, item: &str) -> u32 {
        let inv = &self.0.state.inventory;
        let items = &self.0.state.items;
        inv.main.iter().filter(|s| !s.is_empty() && super::item_key::fits(item, items.name(s.network_id), s.metadata)).map(|s| u32::from(s.count)).sum()
    }

    fn free_slots(&self) -> u32 {
        self.0.state.inventory.main.iter().filter(|s| s.is_empty()).count() as u32
    }

    fn held(&self) -> Option<&str> {
        self.0.state.items.name(self.0.state.inventory.held().network_id)
    }

    fn in_flight(&self, pos: BlockPos) -> bool {
        self.0.placements_in_flight().any(|p| p == pos)
    }

    fn any_in_flight(&self) -> bool {
        !self.0.in_flight.is_empty()
    }

    fn sign_text(&self, pos: BlockPos, front: bool) -> Option<String> {
        self.0.state.block_entities.sign_text(pos, front).map(str::to_string)
    }
}
