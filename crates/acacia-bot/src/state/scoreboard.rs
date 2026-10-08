use std::collections::HashMap;

use acacia_client::proto::packets::{
    RemoveObjective, SetDisplayObjective, SetScore, SetScoreEntriesItemContent as Content,
    SetScoreboardIdentity, SetScoreboardIdentityAction,
};
use acacia_client::proto::{DecodeError, Packet, RawPacket};

use crate::text::strip_formatting;

pub const SLOT_SIDEBAR: &str = "sidebar";
pub const SLOT_LIST: &str = "list";
pub const SLOT_BELOW_NAME: &str = "belowname";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SortOrder {
    Ascending,
    #[default]
    Descending,
}

impl SortOrder {
    fn from_wire(v: i32) -> Self {
        if v == 0 { Self::Ascending } else { Self::Descending }
    }
}

/// One line of an objective, keyed by its scoreboard id in [`Objective::scores`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScoreEntry {
    /// Custom name (with `§` codes) for fake players; empty for player/entity entries.
    pub display: String,
    /// The player/entity this score belongs to; `None` for fake players.
    pub entity_unique_id: Option<i64>,
    pub score: i32,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Objective {
    pub name: String,
    pub display_name: String,
    pub criteria: String,
    pub sort_order: SortOrder,
    pub scores: HashMap<i64, ScoreEntry>,
}

impl Objective {
    fn new(name: &str) -> Self {
        Self { name: name.to_owned(), ..Self::default() }
    }

    /// `(display, score)` in display order: by score per [`SortOrder`], ties by display name.
    pub fn lines(&self) -> Vec<(String, i32)> {
        self.sorted().into_iter().map(|e| (e.display.clone(), e.score)).collect()
    }

    /// The entries in display order (see [`Objective::lines`]).
    pub fn sorted(&self) -> Vec<&ScoreEntry> {
        let mut entries: Vec<_> = self.scores.iter().collect();
        entries.sort_by(|(ida, a), (idb, b)| {
            let by_score = match self.sort_order {
                SortOrder::Ascending => a.score.cmp(&b.score),
                SortOrder::Descending => b.score.cmp(&a.score),
            };
            by_score.then_with(|| a.display.cmp(&b.display)).then_with(|| ida.cmp(idb))
        });
        entries.into_iter().map(|(_, e)| e).collect()
    }
}

#[derive(Debug, Default)]
pub struct Scoreboard {
    objectives: HashMap<String, Objective>,
    /// display slot -> objective name
    slots: HashMap<String, String>,
    /// scoreboard id -> entity unique id (SetScoreboardIdentity)
    identities: HashMap<i64, i64>,
}

impl Scoreboard {
    pub const PACKETS: &'static [u32] =
        &[SetDisplayObjective::ID, RemoveObjective::ID, SetScore::ID, SetScoreboardIdentity::ID];

    pub fn objective(&self, name: &str) -> Option<&Objective> {
        self.objectives.get(name)
    }

    pub fn objectives(&self) -> impl Iterator<Item = &Objective> {
        self.objectives.values()
    }

    pub fn displayed(&self, slot: &str) -> Option<&Objective> {
        self.slots.get(slot).and_then(|name| self.objectives.get(name))
    }

    pub fn sidebar(&self) -> Option<&Objective> {
        self.displayed(SLOT_SIDEBAR)
    }

    /// Sidebar lines in display order with formatting stripped; empty when no sidebar is shown.
    pub fn sidebar_text(&self) -> Vec<String> {
        self.sidebar()
            .map(|o| o.lines().into_iter().map(|(line, _)| strip_formatting(&line)).collect())
            .unwrap_or_default()
    }

    pub fn identity(&self, scoreboard_id: i64) -> Option<i64> {
        self.identities.get(&scoreboard_id).copied()
    }

    pub fn apply(&mut self, packet: &RawPacket) -> Result<(), DecodeError> {
        match packet.id {
            SetDisplayObjective::ID => self.display(packet.decode()?),
            RemoveObjective::ID => self.remove_objective(&packet.decode::<RemoveObjective>()?.objective_name),
            SetScore::ID => self.set_scores(packet.decode()?),
            SetScoreboardIdentity::ID => self.set_identities(packet.decode()?),
            _ => {}
        }
        Ok(())
    }

    fn display(&mut self, p: SetDisplayObjective) {
        if p.objective_name.is_empty() {
            self.slots.remove(&p.display_slot);
            return;
        }
        let o = self.objective_mut(&p.objective_name);
        o.display_name = p.display_name;
        o.criteria = p.criteria_name;
        o.sort_order = SortOrder::from_wire(p.sort_order);
        self.slots.insert(p.display_slot, p.objective_name);
    }

    fn remove_objective(&mut self, name: &str) {
        self.objectives.remove(name);
        self.slots.retain(|_, o| o != name);
    }

    // Objectives are created lazily so scores sent before SetDisplayObjective aren't lost.
    fn objective_mut(&mut self, name: &str) -> &mut Objective {
        self.objectives.entry(name.to_owned()).or_insert_with(|| Objective::new(name))
    }

    fn set_scores(&mut self, p: SetScore) {
        for item in p.entries {
            let id = item.scoreboard_id;
            let (objective, entry) = match item.content {
                Content::Remove(r) => {
                    self.remove_score(id, r.objective_name.as_deref());
                    continue;
                }
                Content::FakePlayer(f) => {
                    (f.objective_name, ScoreEntry { display: f.custom_name, entity_unique_id: None, score: f.score })
                }
                Content::Default(d) => (
                    d.objective_name,
                    ScoreEntry { display: String::new(), entity_unique_id: Some(d.entity_unique_id), score: d.score },
                ),
            };
            self.objective_mut(&objective).scores.insert(id, entry);
        }
    }

    /// Scoreboard ids are global, so a remove without an objective name clears the id everywhere.
    fn remove_score(&mut self, id: i64, objective: Option<&str>) {
        match objective {
            Some(name) => {
                if let Some(o) = self.objectives.get_mut(name) {
                    o.scores.remove(&id);
                }
            }
            None => self.objectives.values_mut().for_each(|o| {
                o.scores.remove(&id);
            }),
        }
    }

    fn set_identities(&mut self, p: SetScoreboardIdentity) {
        for e in p.entries {
            match (p.action, e.entity_unique_id) {
                (SetScoreboardIdentityAction::RegisterIdentity, Some(uid)) => {
                    self.identities.insert(e.scoreboard_id, uid);
                }
                (SetScoreboardIdentityAction::ClearIdentity, _) => {
                    self.identities.remove(&e.scoreboard_id);
                }
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests;
