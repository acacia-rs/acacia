//! Which server positions corrected a Java player: every one after the first, except a respawn's placing, the
//! teleport `startRiding` sends just before the seat, and the exit teleport `dismountTo` sends just after the seat
//! is given up.

#[derive(Debug, Default)]
pub(crate) struct Setbacks {
    pub(crate) teleports: u32,
    pub(crate) last: Option<u64>,
    /// `last` before the latest teleport, restored when that teleport only seated the player.
    before: Option<u64>,
    /// A respawn's new player has not ticked yet: positions until then place it (Paper's respawn sends two).
    pub(crate) respawned: bool,
    /// The tick the server's passenger lists last unseated the player.
    unseated: Option<u64>,
    seated: bool,
    pub(crate) count: u32,
}

impl Setbacks {
    /// A `PlayerPosition` at client tick `tick`; true when it counts as a setback.
    pub(crate) fn on_position(&mut self, tick: u64) -> bool {
        let first = self.teleports == 0;
        self.teleports += 1;
        // Paper sends SetPassengers then the exit position in one server tick; a client tick may fall between.
        if self.unseated.take().is_some_and(|t| tick <= t + 1) || first || self.respawned {
            return false;
        }
        (self.before, self.last) = (self.last, Some(tick));
        self.count += 1;
        true
    }

    /// A `SetPassengers` after the client took its seat from it: a boarding cancels the setback its teleport counted.
    pub(crate) fn on_seat(&mut self, seated: bool, tick: u64) {
        if seated && !self.seated && self.last == Some(tick) {
            self.last = self.before;
            self.count -= 1;
        }
        if self.seated && !seated {
            self.unseated = Some(tick);
        }
        self.seated = seated;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boarding_and_dismount_teleports_are_not_setbacks() {
        let mut s = Setbacks::default();
        assert!(!s.on_position(1));
        assert!(s.on_position(10));
        s.on_seat(true, 10);
        s.on_seat(false, 50);
        assert!(!s.on_position(51));
        assert!(s.on_position(60));
        assert_eq!((s.count, s.last), (1, Some(60)));
    }
}
