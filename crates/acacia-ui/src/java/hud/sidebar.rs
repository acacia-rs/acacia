//! `Hud.displayScoreboardSidebar`, with the slice of the scoreboard it reads: scores, number
//! formats and team affixes. Ported from Horizon's `hud_overlays.rs` and `scoreboard.rs` (MIT).

use super::sort::{compare_ignore_case, java_sort};
use crate::java::gui::{Gui, Metrics};
use crate::java::mth::WHITE;
use crate::java::text::{COLORS, Style, Text};

/// A `NumberFormat`.
#[derive(Clone, Debug, PartialEq)]
pub enum NumberFormat {
    Blank,
    Fixed(Text),
    Styled(Style),
}

/// `StyledFormat.SIDEBAR_DEFAULT`: red.
pub fn sidebar_default() -> NumberFormat {
    NumberFormat::Styled(Style::color(COLORS[12].2))
}

impl NumberFormat {
    pub fn format(&self, value: i32) -> Text {
        match self {
            NumberFormat::Blank => Text::default(),
            NumberFormat::Fixed(t) => t.clone(),
            NumberFormat::Styled(s) => Text::styled(value.to_string(), s),
        }
    }
}

/// A `PlayerScoreEntry`.
#[derive(Clone, Debug, PartialEq)]
pub struct ScoreEntry {
    pub owner: String,
    pub value: i32,
    pub display: Option<Text>,
    pub format: Option<NumberFormat>,
}

impl ScoreEntry {
    /// Owners starting with `#` are not listed.
    pub fn is_hidden(&self) -> bool {
        self.owner.starts_with('#')
    }

    pub fn owner_name(&self) -> Text {
        self.display.clone().unwrap_or_else(|| Text::literal(self.owner.clone()))
    }
}

/// A `PlayerTeam`'s name decorations.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Team {
    pub name: String,
    pub prefix: Text,
    pub suffix: Text,
    /// The `TeamColor`'s RGB.
    pub color: Option<u32>,
}

impl Team {
    /// `getFormattedName`: prefix, name and suffix, in the team colour where they set none.
    pub fn formatted_name(&self, name: Text) -> Text {
        let t = self.prefix.clone().append(name).append(self.suffix.clone());
        match self.color {
            Some(c) => t.with_color(c),
            None => t,
        }
    }
}

/// The objective as the sidebar shows it.
pub struct Sidebar<'a> {
    pub title: &'a Text,
    /// `numberFormat`, `None` for the sidebar default.
    pub format: Option<&'a NumberFormat>,
    /// `listPlayerScores` in the scoreboard's order.
    pub scores: &'a [ScoreEntry],
    /// `getPlayersTeam`.
    pub team: &'a dyn Fn(&str) -> Option<&'a Team>,
    /// `getBackgroundColor(0.3F)` and `(0.4F)`.
    pub background: u32,
    pub header: u32,
}

/// `Hud.displayScoreboardSidebar`.
pub fn sidebar(gui: &mut Gui, font: &dyn Metrics, s: &Sidebar) {
    let default = sidebar_default();
    let format = s.format.unwrap_or(&default);
    let mut entries: Vec<&ScoreEntry> = s.scores.iter().filter(|e| !e.is_hidden()).collect();
    java_sort(&mut entries, |a, b| b.value.cmp(&a.value).then_with(|| compare_ignore_case(&a.owner, &b.owner)));
    entries.truncate(15);
    let rows: Vec<(Text, Text, i32)> = entries
        .iter()
        .map(|e| {
            let name = match (s.team)(&e.owner) {
                Some(team) => team.formatted_name(e.owner_name()),
                None => e.owner_name(),
            };
            let score = e.format.as_ref().unwrap_or(format).format(e.value);
            let width = font.width(&score);
            (name, score, width)
        })
        .collect();
    let title_width = font.width(s.title);
    let spacer = font.width(&Text::literal(": "));
    let biggest = rows.iter().fold(title_width, |m, (name, _, sw)| m.max(font.width(name) + if *sw > 0 { spacer + sw } else { 0 }));
    let count = rows.len() as i32;
    let bottom = gui.height / 2 + count * 9 / 3;
    let left = gui.width - biggest - 3;
    let right = gui.width - 3 + 2;
    let header_y = bottom - count * 9;
    gui.fill(left - 2, header_y - 9 - 1, right, header_y - 1, s.header);
    gui.fill(left - 2, header_y - 1, right, bottom, s.background);
    gui.text(s.title.clone(), left + biggest / 2 - title_width / 2, header_y - 9, WHITE, false);
    for (i, (name, score, score_width)) in rows.into_iter().enumerate() {
        let y = bottom - (count - i as i32) * 9;
        gui.text(name, left, y, WHITE, false);
        gui.text(score, right - score_width, y, WHITE, false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::java::gui::{Fixed, GuiDraw};

    fn entry(owner: &str, value: i32) -> ScoreEntry {
        ScoreEntry { owner: owner.into(), value, display: None, format: None }
    }

    #[test]
    fn highest_first_hidden_left_out() {
        let scores = [entry("bob", 1), entry("#x", 9), entry("Alex", 1), entry("Zed", 5)];
        let none = |_: &str| -> Option<&Team> { None };
        let title = Text::literal("Kills");
        let mut gui = Gui::new(320, 240);
        sidebar(&mut gui, &Fixed(6.0), &Sidebar { title: &title, format: None, scores: &scores, team: &none, background: 0, header: 0 });
        let names: Vec<String> = gui.draws.iter().filter_map(|d| match d {
            GuiDraw::Text { text, shadow: false, .. } => Some(text.plain()),
            _ => None,
        }).collect();
        assert_eq!(names, ["Kills", "Zed", "5", "Alex", "1", "bob", "1"]);
    }

    #[test]
    fn team_colour_under_the_affixes() {
        let team = Team { name: "a".into(), prefix: Text::literal("[A] ").with_color(0xFF5555), suffix: Text::default(), color: Some(0x5555FF) };
        let name = team.formatted_name(Text::literal("Mia"));
        assert_eq!(name.spans[0].style.color, Some(0xFF5555));
        assert_eq!(name.spans[1].style.color, Some(0x5555FF));
    }
}
