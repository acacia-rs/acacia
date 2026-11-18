//! Objectives, scores, display slots and teams.

use super::{NullableString, OptionalText};
use crate::text::{Component, NumberFormat};
use crate::types::{ObjectiveRenderType, var_int_enum};
use crate::wire::read_len;
use crate::{Error, VarInt, Wire};

/// `ClientboundSetObjectivePacket`: the name, a method byte (add 0, remove 1, change 2), and for add and change
/// the display name, render type and an optional number format.
#[derive(Debug, Clone, PartialEq)]
pub enum ObjectiveMethod {
    Add(ObjectiveDisplay),
    Remove,
    Change(ObjectiveDisplay),
}

body!(ObjectiveDisplay { name: Component, render_type: ObjectiveRenderType, number_format: Option<NumberFormat> });

body!(SetObjective { objective: String, method: ObjectiveMethod });

impl Wire for ObjectiveMethod {
    fn read(input: &mut &[u8]) -> Result<Self, Error> {
        match u8::read(input)? {
            0 => ObjectiveDisplay::read(input).map(Self::Add),
            1 => Ok(Self::Remove),
            2 => ObjectiveDisplay::read(input).map(Self::Change),
            _ => Err(Error::Invalid("objective method")),
        }
    }

    fn write(&self, out: &mut Vec<u8>) {
        match self {
            Self::Add(display) => {
                0u8.write(out);
                display.write(out);
            }
            Self::Remove => 1u8.write(out),
            Self::Change(display) => {
                2u8.write(out);
                display.write(out);
            }
        }
    }
}

// `display` replaces the shown owner name; `number_format` the shown score.
body!(SetScore { owner: String, objective: String, score: VarInt, display: OptionalText, number_format: Option<NumberFormat> });
// A missing objective resets the owner's score in all of them.
body!(ResetScore { owner: String, objective: NullableString });

var_int_enum!(
    /// `TeamColor`, in registration order; also the colour of a team's sidebar slot.
    TeamColor, "team colour",
    [Black, DarkBlue, DarkGreen, DarkAqua, DarkRed, DarkPurple, Gold, Gray, DarkGray, Blue, Green, Aqua, Red, LightPurple, Yellow, White]
);

/// `DisplaySlot`: list 0, sidebar 1, below name 2, then the sixteen team sidebars.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplaySlot {
    List,
    Sidebar,
    BelowName,
    Team(TeamColor),
}

impl DisplaySlot {
    pub fn id(self) -> i32 {
        match self {
            Self::List => 0,
            Self::Sidebar => 1,
            Self::BelowName => 2,
            Self::Team(color) => 3 + color as i32,
        }
    }
}

impl Wire for DisplaySlot {
    fn read(input: &mut &[u8]) -> Result<Self, Error> {
        match VarInt::read(input)?.0 {
            0 => Ok(Self::List),
            1 => Ok(Self::Sidebar),
            2 => Ok(Self::BelowName),
            n @ 3..=18 => Ok(Self::Team(TeamColor::ALL[(n - 3) as usize])),
            _ => Err(Error::Invalid("display slot")),
        }
    }

    fn write(&self, out: &mut Vec<u8>) {
        VarInt(self.id()).write(out);
    }
}

// An empty objective name clears the slot.
body!(SetDisplayObjective { slot: DisplaySlot, objective: String });

var_int_enum!(
    /// `Team.Visibility`.
    NameTagVisibility, "name tag visibility",
    [Always, Never, HideForOtherTeams, HideForOwnTeam]
);
var_int_enum!(
    /// `Team.CollisionRule`.
    CollisionRule, "collision rule",
    [Always, Never, PushOtherTeams, PushOwnTeam]
);

/// `TeamColor` behind a presence flag: no colour is vanilla's reset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct OptionalTeamColor(pub Option<TeamColor>);

impl Wire for OptionalTeamColor {
    fn read(input: &mut &[u8]) -> Result<Self, Error> {
        if bool::read(input)? { TeamColor::read(input).map(|c| Self(Some(c))) } else { Ok(Self(None)) }
    }

    fn write(&self, out: &mut Vec<u8>) {
        self.0.is_some().write(out);
        if let Some(color) = self.0 {
            color.write(out);
        }
    }
}

body!(
    /// `options`: bit 0 allows friendly fire, bit 1 shows invisible teammates.
    TeamParameters {
        display_name: Component,
        prefix: Component,
        suffix: Component,
        name_tag_visibility: NameTagVisibility,
        collision_rule: CollisionRule,
        color: OptionalTeamColor,
        options: u8,
    }
);

/// `ClientboundSetPlayerTeamPacket`: the method byte picks the body. Parameters follow for add and change, the
/// member list for add, join and leave.
#[derive(Debug, Clone, PartialEq)]
pub enum TeamAction {
    Add { parameters: TeamParameters, players: Vec<String> },
    Remove,
    Change(TeamParameters),
    Join(Vec<String>),
    Leave(Vec<String>),
}

body!(SetPlayerTeam { team: String, action: TeamAction });

impl Wire for TeamAction {
    fn read(input: &mut &[u8]) -> Result<Self, Error> {
        let strings = |input: &mut &[u8]| (0..read_len(input)?).map(|_| String::read(input)).collect::<Result<Vec<_>, _>>();
        match u8::read(input)? {
            0 => Ok(Self::Add { parameters: TeamParameters::read(input)?, players: strings(input)? }),
            1 => Ok(Self::Remove),
            2 => TeamParameters::read(input).map(Self::Change),
            3 => strings(input).map(Self::Join),
            4 => strings(input).map(Self::Leave),
            _ => Err(Error::Invalid("team method")),
        }
    }

    fn write(&self, out: &mut Vec<u8>) {
        match self {
            Self::Add { parameters, players } => {
                0u8.write(out);
                parameters.write(out);
                players.write(out);
            }
            Self::Remove => 1u8.write(out),
            Self::Change(parameters) => {
                2u8.write(out);
                parameters.write(out);
            }
            Self::Join(players) => {
                3u8.write(out);
                players.write(out);
            }
            Self::Leave(players) => {
                4u8.write(out);
                players.write(out);
            }
        }
    }
}
