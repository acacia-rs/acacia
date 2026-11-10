//! Placing while driving on ([`super::super::Process::en_route`]): on a plain walking step the look turns to the
//! click (the keys keep the course), the next tick sends the click, then two ticks only walk. One click in three
//! ticks is the vanilla client's cadence while moving; a click in its aim tick or two in a tick made BDS drop
//! placements silently (docs/builder.md "Tried and dropped"). Design: research/build-throughput-deep.md §2.3.

use super::super::{act, Act, ProcId};
use super::{bounded, needs_stack_request, Processes};
use crate::pathfind::MoveKind;
use crate::{ActionError, Bot};

/// Ticks from one en-route click to the next aim.
const EVERY: u64 = 3;

#[derive(Default)]
pub(super) struct Walk {
    tick: u64,
    /// The placement aimed at last tick, clicked this one.
    aimed: Option<(ProcId, Act)>,
    next_at: u64,
}

impl Walk {
    /// A tick that does not drive: an aimed placement is not clicked later from elsewhere.
    pub(super) fn drop_aim(&mut self) {
        self.aimed = None;
    }
}

impl Processes {
    /// After a driving tick's steering (`moving`: the pilot is walking a path): aims or clicks the process's
    /// en-route placement.
    pub(super) async fn place_en_route(&mut self, bot: &mut Bot, moving: bool) -> Result<(), ActionError> {
        self.walk.tick += 1;
        let Some(course) = bot.backend.movement().map(|m| m.controls) else { return Ok(()) };
        if let Some((id, a)) = self.walk.aimed.take() {
            if !moving {
                return Ok(());
            }
            // Re-aimed from this tick's eye first: if the place still turns enough to wait a tick, that tick's
            // keys keep the course too.
            aim(bot, &a, &course);
            match bounded(act::send_place(bot, &a)).await {
                Ok(target) => {
                    self.sent.insert(target, (id, a));
                }
                Err(ActionError::Disconnected) => return Err(ActionError::Disconnected),
                Err(e) => self.arbiter.acted(id, &a, &Err(e)),
            }
            if let Some(m) = bot.backend.movement_mut() {
                m.controls.keep_course(&course);
            }
            self.walk.next_at = self.walk.tick + EVERY;
            return Ok(());
        }
        let next = self.arbiter.pilot().and_then(|p| p.follower()).and_then(|f| f.ahead().first()).map(|n| (n.kind, n.work.is_empty(), n.mac.is_some()));
        let plain = next.is_some_and(|(kind, no_work, mac)| kind == MoveKind::Walk && no_work && !mac);
        let ground = bot.backend.movement().is_some_and(|m| m.on_ground());
        // Asked only when it can be aimed now: taking it claims the cell.
        if !moving || !plain || !ground || self.walk.tick < self.walk.next_at || !bot.governor.may_send() {
            tracing::debug!(moving, ?next, ground, wait = self.walk.tick < self.walk.next_at, send = bot.governor.may_send(), "en route: not now");
            return Ok(());
        }
        let Some((id, a)) = self.arbiter.en_route() else { return Ok(()) };
        if needs_stack_request(bot, &a) {
            return Ok(());
        }
        aim(bot, &a, &course);
        self.walk.aimed = Some((id, a));
        Ok(())
    }
}

/// Looks at `a`'s click point from the eye now, keeping `course`'s walking direction.
fn aim(bot: &mut Bot, a: &Act, course: &crate::movement::Controls) {
    let Act::Place { against, click, .. } = a else { return };
    let eye = bot.eye_position();
    let point = [against[0] as f32 + click[0], against[1] as f32 + click[1], against[2] as f32 + click[2]];
    if let Some(mv) = bot.backend.movement_mut() {
        mv.controls.look_at(eye, point);
        mv.controls.keep_course(course);
    }
}

#[cfg(test)]
mod tests {
    use crate::movement::Controls;

    fn keys(course_yaw: f32, forward: f32, strafe: f32, look: f32) -> (f32, f32) {
        let course = Controls { yaw: course_yaw, forward, strafe, sprint: true, ..Controls::default() };
        let mut c = Controls { yaw: look, ..course };
        c.keep_course(&course);
        assert!(!c.sprint || (forward == 0.0 && strafe == 0.0), "sprint needs forward");
        (c.forward, c.strafe)
    }

    #[test]
    fn keys_keep_the_course_while_looking_away() {
        assert_eq!(keys(0.0, 1.0, 0.0, 0.0), (1.0, 0.0), "looking where it walks");
        assert_eq!(keys(0.0, 1.0, 0.0, 90.0), (0.0, 1.0), "course 90° left of the look");
        assert_eq!(keys(0.0, 1.0, 0.0, 180.0), (-1.0, 0.0), "walking backwards");
        assert_eq!(keys(0.0, 1.0, 0.0, 45.0), (1.0, 1.0), "diagonal");
        assert_eq!(keys(90.0, 1.0, -1.0, 90.0), (1.0, -1.0), "a diagonal course kept");
        assert_eq!(keys(0.0, 0.0, 0.0, 120.0), (0.0, 0.0), "standing still stays still");
    }
}
