//! The entities the pilot plans round: what its profile lets it know of them (`hazard::perceive`).

use std::sync::Arc;

use super::Pilot;
use crate::pathfind::hazard::{self, Hazards, Threats, View};

impl Pilot {
    /// This tick's field from the `threats` around the bot, as `view` perceives them. Once a tick, before
    /// [`Pilot::drive`] (its `SearchOpts::hazards`).
    /// Again within a tick (same `threats.at`), returns that tick's field.
    pub fn perceive(&mut self, threats: Threats, view: &View) -> Option<Arc<Hazards>> {
        if self.perceived_at.replace(threats.at) == Some(threats.at) {
            return self.field.clone();
        }
        let profile = self.opts.search.profile;
        self.field = hazard::field(&self.senses.filter(threats, view, profile), profile);
        self.field.clone()
    }

    /// The field of the last [`Pilot::perceive`].
    pub fn field(&self) -> Option<&Hazards> {
        self.field.as_deref()
    }

    /// Takes over what `old` (a pilot for an earlier goal) knew of the entities.
    pub(crate) fn inherit(&mut self, old: Pilot) {
        (self.senses, self.field, self.perceived_at) = (old.senses, old.field, old.perceived_at);
    }
}
