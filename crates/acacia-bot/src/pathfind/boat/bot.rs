//! [`Bot::boat_to`]: a boat leg driven live: board, paddle with a [`BoatPilot`] (re-planned while the far water is
//! still unknown), get off at the stop, break and pick up the boat, walk the rest.

use acacia_physics::BlockPos;

use super::pilot::{BoatOrder, BoatPilot};
use super::{BoatOpts, Leg, plan_leg};
use crate::human;
use crate::pathfind::profile::MovementProfile;
use crate::pathfind::terrain::Terrain;
use crate::pathfind::{Goal, GotoOpts};
use crate::{ActionError, Bot};

/// Ticks between re-plans while the route stops short of the target.
const REPLAN_TICKS: u64 = 20;
/// Ticks the boat's simulation may take to start after boarding.
const SEATED_TICKS: u32 = 20;
/// Ticks seated before the first paddle. BDS's paddle clock starts near boarding at an unknown tick and counts no
/// stroke fresh for its first ten ticks; a first pull after that is fresh on both sides, keeping the strokes in phase.
const SETTLE_TICKS: u32 = 12;
/// Off the boat further than this from the target, the bot walks the rest.
const WALK_REST: f32 = 1.0;

impl Bot {
    /// Crosses still water by boat to `target` (feet block on the far bank; physics bots with the entity tracker): boards an empty boat
    /// in reach or places one from the hotbar, paddles, gets off by the target, and breaks and collects the boat
    /// unless `opts.keep_boat`.
    pub async fn boat_to(&mut self, target: BlockPos, opts: BoatOpts) -> Result<(), ActionError> {
        if self.movement.as_ref().is_none_or(|m| !m.is_started()) {
            return Err(ActionError::NotPossible("boating needs a physics bot (BotConfig.physics)".into()));
        }
        if !self.state.tracks_entities() {
            return Err(ActionError::NotPossible("boating needs the entity tracker (BotConfig.trackers.entities)".into()));
        }
        let boat = self.board_boat(target).await?;
        for _ in 0..SEATED_TICKS {
            if self.boat_state().is_some() {
                break;
            }
            self.next_tick(|_, _| false).await?;
        }
        for _ in 0..SETTLE_TICKS {
            self.next_tick(|_, _| false).await?;
        }
        let paddled = self.paddle(target, &opts).await;
        if let Some(m) = self.movement.as_mut() {
            m.controls.stop();
        }
        if let Err(e) = paddled {
            return Err(e);
        }
        if opts.profile == MovementProfile::Human {
            let (lo, hi) = human::BOAT_EXIT_PAUSE;
            for _ in 0..lo + (crate::cadence::seed() % u64::from(hi - lo + 1)) as u32 {
                self.next_tick(|_, _| false).await?;
            }
        }
        self.dismount().await?;
        if !opts.keep_boat {
            self.collect_boat(boat).await?;
        }
        let Some(feet) = self.movement.as_ref().and_then(|m| m.position()) else { return Ok(()) };
        let centre = [target[0] as f32 + 0.5, target[2] as f32 + 0.5];
        if (feet[0] - centre[0]).hypot(feet[2] - centre[1]) > WALK_REST || feet[1].floor() as i32 != target[1] {
            self.goto_with(Goal::Block(target), GotoOpts::default()).await?;
        }
        Ok(())
    }

    /// Paddles the boat the bot sits in until it rests at a stop by `target`.
    async fn paddle(&mut self, target: BlockPos, opts: &BoatOpts) -> Result<(), ActionError> {
        let seat = self.state.riding.seat_offset.as_ref().map_or([0.0; 3], |o| [o.x, o.y, o.z]);
        let mut pilot: Option<BoatPilot> = None;
        let mut planned_at = 0;
        for tick in 0.. {
            let st = self.boat_state().ok_or_else(|| ActionError::NotPossible("not driving a boat".into()))?;
            let order = {
                let (Some(world), Some(movement)) = (self.world.as_ref(), self.movement.as_mut()) else { return Err(ActionError::Disconnected) };
                let (Some(view), Some(registry)) = (world.view(), world.registry()) else { return Err(ActionError::Disconnected) };
                let terrain = Terrain::new(view, registry.as_ref());
                let replan = pilot.as_ref().is_none_or(|p| !p.complete() && tick - planned_at >= REPLAN_TICKS);
                if replan {
                    planned_at = tick;
                    match (plan_leg(&terrain, st.pos, target, seat, opts.profile), pilot.as_mut()) {
                        (Some(leg), Some(p)) => p.reroute(&leg),
                        (Some(leg), None) => {
                            let Leg { route, exit, .. } = &leg;
                            tracing::debug!(points = route.points.len(), length = route.length(), complete = route.complete, ?exit, "boat leg");
                            pilot = Some(BoatPilot::new(&leg, seat, opts.profile, opts.max_ticks, crate::cadence::seed()));
                        }
                        (None, None) => return Err(ActionError::NotPossible(format!("no water route from {:?} towards {target:?}", st.pos))),
                        (None, Some(_)) => {}
                    }
                }
                let Some(p) = pilot.as_mut() else { return Err(ActionError::Disconnected) };
                p.tick(&st, &terrain, &mut movement.controls)
            };
            match order {
                BoatOrder::Paddle => {}
                BoatOrder::Arrived => return Ok(()),
                BoatOrder::Fail(why) => return Err(ActionError::NotPossible(why)),
            }
            self.next_tick(|_, _| false).await?;
        }
        Ok(())
    }
}
