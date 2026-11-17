//! Scoreboard teams (`ClientboundSetPlayerTeamPacket`), kept for their collision rules: who may push whom.

use std::collections::HashMap;

use acacia_java_proto::ids::play::clientbound as from;
use acacia_java_proto::{read_varint, Error, Wire};

use crate::messages::component;

const METHOD_ADD: u8 = 0;
const METHOD_REMOVE: u8 = 1;
const METHOD_CHANGE: u8 = 2;
const METHOD_JOIN: u8 = 3;
const METHOD_LEAVE: u8 = 4;

/// `Team.CollisionRule`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CollisionRule {
    #[default]
    Always,
    Never,
    PushOtherTeams,
    PushOwnTeam,
}

impl CollisionRule {
    /// `ByIdMap.continuous` with `OutOfBoundsStrategy.ZERO`.
    fn from_id(id: i32) -> Self {
        match id {
            1 => Self::Never,
            2 => Self::PushOtherTeams,
            3 => Self::PushOwnTeam,
            _ => Self::Always,
        }
    }
}

/// Team rules by name and each member's team. Members are player names, or other entities' UUID strings.
#[derive(Clone, Debug, Default)]
pub struct Teams {
    rules: HashMap<String, CollisionRule>,
    member_of: HashMap<String, String>,
}

impl Teams {
    /// `member`'s team and its rule.
    pub fn team_of(&self, member: &str) -> Option<(&str, CollisionRule)> {
        let team = self.member_of.get(member)?;
        Some((team, self.rules.get(team).copied().unwrap_or_default()))
    }

    /// `EntitySelector.pushableBy(pusher)` on the client, for the local player as the one pushed.
    pub fn pushes(&self, pusher: &str, local: &str) -> bool {
        use CollisionRule::*;
        let own = self.team_of(pusher);
        let own_rule = own.map_or(Always, |(_, r)| r);
        if own_rule == Never {
            return false;
        }
        let theirs = self.team_of(local);
        let their_rule = theirs.map_or(Always, |(_, r)| r);
        if their_rule == Never {
            return false;
        }
        let same_team = own.is_some() && own.map(|(t, _)| t) == theirs.map(|(t, _)| t);
        if (own_rule == PushOwnTeam || their_rule == PushOwnTeam) && same_team {
            return false;
        }
        (own_rule != PushOtherTeams && their_rule != PushOtherTeams) || same_team
    }

    pub(crate) fn apply(&mut self, id: i32, mut input: &[u8]) -> Result<(), Error> {
        let input = &mut input;
        match id {
            from::LOGIN => *self = Teams::default(),
            from::SET_PLAYER_TEAM => self.set_player_team(input)?,
            _ => {}
        }
        Ok(())
    }

    fn set_player_team(&mut self, input: &mut &[u8]) -> Result<(), Error> {
        let name = String::read(input)?;
        let method = u8::read(input)?;
        if method == METHOD_ADD || method == METHOD_CHANGE {
            // Display name, prefix and suffix, then the name tag visibility.
            for _ in 0..3 {
                component(input)?;
            }
            read_varint(input)?;
            self.rules.insert(name.clone(), CollisionRule::from_id(read_varint(input)?));
        }
        if method == METHOD_REMOVE {
            self.rules.remove(&name);
            self.member_of.retain(|_, team| *team != name);
        }
        if method == METHOD_ADD || method == METHOD_JOIN || method == METHOD_LEAVE {
            for member in <Vec<String>>::read(input)? {
                if method == METHOD_LEAVE {
                    self.member_of.remove(&member);
                } else {
                    self.member_of.insert(member, name.clone());
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn teams(entries: &[(&str, CollisionRule, &[&str])]) -> Teams {
        let mut t = Teams::default();
        for (team, rule, members) in entries {
            t.rules.insert((*team).into(), *rule);
            for m in *members {
                t.member_of.insert((*m).into(), (*team).into());
            }
        }
        t
    }

    #[test]
    fn collision_rules() {
        use CollisionRule::*;
        assert!(Teams::default().pushes("a", "me"));
        assert!(!teams(&[("t", Never, &["a"])]).pushes("a", "me"));
        assert!(!teams(&[("t", Never, &["me"])]).pushes("a", "me"));
        assert!(!teams(&[("t", PushOwnTeam, &["a", "me"])]).pushes("a", "me"));
        assert!(teams(&[("t", PushOwnTeam, &["a"])]).pushes("a", "me"));
        assert!(!teams(&[("t", PushOtherTeams, &["a"])]).pushes("a", "me"));
        assert!(teams(&[("t", PushOtherTeams, &["a", "me"])]).pushes("a", "me"));
    }
}
