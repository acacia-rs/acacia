//! `PlayerTabOverlay`: the player list. Listed players sorted by list order, spectators last,
//! team and name; columns of at most 20 rows; heads, names (team affixes and colour, italic
//! spectators), ping bars, the `list` objective (scores in their number format, or hearts) and the
//! header and footer. Ported from Horizon's `tab_list.rs` (MIT).

use std::collections::HashMap;

use serde_json::{Value, json};

use super::sidebar::{NumberFormat, Team};
use super::sort::compare_ignore_case;
use super::tab_hearts::{self, HealthState};
use crate::java::gui::{Blend, Gui, GuiDraw, Metrics};
use crate::java::mth::WHITE;
use crate::java::split::split_lines;
use crate::java::text::{COLORS, Lang, Style, Text};

/// `GameType.SPECTATOR`'s id.
pub const SPECTATOR: i32 = 3;

/// A `PlayerInfo` the list shows.
#[derive(Clone, Debug, PartialEq)]
pub struct TabPlayer {
    pub uuid: u128,
    /// The profile name: the score holder and team member.
    pub name: String,
    /// `GameType` id.
    pub game_mode: i32,
    /// Milliseconds; negative when unknown.
    pub latency: i32,
    /// `tabListDisplayName` as component JSON: a spectator's is made italic at its root.
    pub display_name: Option<Value>,
    pub list_order: i32,
    pub show_hat: bool,
    /// The skin's texture under `textures/` without `.png`.
    pub skin: String,
    /// In the level as Dinnerbone or Grumm (`AvatarRenderer.isPlayerUpsideDown`).
    pub upside_down: bool,
}

/// `DefaultPlayerSkin.get(UUID)`: by `UUID.hashCode`.
pub fn default_skin(uuid: u128) -> String {
    const SKINS: [&str; 9] = ["alex", "ari", "efe", "kai", "makena", "noor", "steve", "sunny", "zuri"];
    let hilo = ((uuid >> 64) as u64) ^ (uuid as u64);
    let n = (((hilo >> 32) as i32) ^ (hilo as i32)).rem_euclid(18) as usize;
    format!("entity/player/{}/{}", if n < 9 { "slim" } else { "wide" }, SKINS[n % 9])
}

/// `PlayerTabOverlay.extractPingIcon`'s sprite.
pub fn ping_sprite(latency: i32) -> &'static str {
    match latency {
        i32::MIN..=-1 => "icon/ping_unknown",
        0..150 => "icon/ping_5",
        150..300 => "icon/ping_4",
        300..600 => "icon/ping_3",
        600..1000 => "icon/ping_2",
        _ => "icon/ping_1",
    }
}

/// `Options.getBackgroundColor(553648127)`: the row background.
pub fn row_background(chat_only: bool, opacity: f32) -> u32 {
    if chat_only { 0x20FF_FFFF } else { crate::java::mth::color_from_float(opacity, 0.0, 0.0, 0.0) }
}

/// `withStyle(ITALIC)` on a component's root.
fn italic_root(c: &Value) -> Value {
    match c {
        Value::Object(o) => {
            let mut o = o.clone();
            o.insert("italic".into(), Value::Bool(true));
            Value::Object(o)
        }
        Value::Array(list) if !list.is_empty() => {
            let mut list = list.clone();
            list[0] = italic_root(&list[0]);
            Value::Array(list)
        }
        Value::String(s) => json!({"text": s, "italic": true}),
        other => json!({"text": other.to_string(), "italic": true}),
    }
}

/// The `list` objective.
#[derive(Clone, Debug, PartialEq)]
pub struct ListObjective {
    pub hearts: bool,
    pub format: Option<NumberFormat>,
}

/// A score's value and number format override.
pub type ScoreInfo = (i32, Option<NumberFormat>);

/// What a frame reads besides the overlay's own state.
pub struct TabInputs<'a> {
    /// `getListedOnlinePlayers` in listing order.
    pub players: &'a [TabPlayer],
    pub team: &'a dyn Fn(&str) -> Option<&'a Team>,
    /// `getPlayerScoreInfo`: value and number format override.
    pub score: &'a dyn Fn(&str) -> Option<ScoreInfo>,
    pub objective: Option<&'a ListObjective>,
    /// `ClientPacketListener.onlineMode`: heads are shown.
    pub online_mode: bool,
    /// `Hud.getGuiTicks`.
    pub tick: i64,
    /// [`row_background`].
    pub background: u32,
    pub lang: Lang<'a>,
}

/// `PlayerTabOverlay`'s header, footer, visibility and heart states.
#[derive(Clone, Debug, Default)]
pub struct TabList {
    pub header: Option<Text>,
    pub footer: Option<Text>,
    visible: bool,
    health: HashMap<u128, HealthState>,
}

impl TabList {
    /// `setVisible`: a change forgets the heart states.
    pub fn set_visible(&mut self, visible: bool) {
        if self.visible != visible {
            self.health.clear();
            self.visible = visible;
        }
    }

    /// `getPlayerInfos`: `PLAYER_COMPARATOR` order, at most 80.
    pub fn sorted<'p>(i: &TabInputs<'p>) -> Vec<&'p TabPlayer> {
        let team = |p: &TabPlayer| (i.team)(&p.name).map_or("", |t| t.name.as_str()).encode_utf16().collect::<Vec<u16>>();
        let mut out: Vec<&TabPlayer> = i.players.iter().collect();
        out.sort_by(|a, b| {
            a.list_order
                .wrapping_neg()
                .cmp(&b.list_order.wrapping_neg())
                .then_with(|| (a.game_mode == SPECTATOR).cmp(&(b.game_mode == SPECTATOR)))
                .then_with(|| team(a).cmp(&team(b)))
                .then_with(|| compare_ignore_case(&a.name, &b.name))
        });
        out.truncate(80);
        out
    }

    /// `getNameForDisplay`.
    pub fn name_for_display(p: &TabPlayer, i: &TabInputs) -> Text {
        let spectator = p.game_mode == SPECTATOR;
        if let Some(d) = &p.display_name {
            return Text::from_json(&if spectator { italic_root(d) } else { d.clone() }, i.lang);
        }
        let name = Text::literal(p.name.clone());
        let name = match (i.team)(&p.name) {
            Some(team) => team.formatted_name(name),
            None => name,
        };
        if spectator { name.with_style(&Style { italic: Some(true), ..Style::default() }) } else { name }
    }

    /// `extractRenderState` for a `screen_width` wide screen.
    pub fn extract(&mut self, gui: &mut Gui, font: &dyn Metrics, i: &TabInputs, screen_width: i32) {
        let players = Self::sorted(i);
        let spacer = font.width(&Text::literal(" "));
        let (mut max_name, mut max_score) = (0, 0);
        let default = NumberFormat::Styled(Style::color(COLORS[14].2));
        let mut entries = Vec::with_capacity(players.len());
        for p in &players {
            let name = Self::name_for_display(p, i);
            max_name = max_name.max(font.width(&name));
            let (mut score, mut formatted, mut score_width) = (0, None, 0);
            if let Some(objective) = i.objective {
                let info = (i.score)(&p.name);
                if let Some((value, _)) = &info {
                    score = *value;
                }
                if !objective.hearts {
                    if let Some((value, own)) = &info {
                        let text = own.as_ref().or(objective.format.as_ref()).unwrap_or(&default).format(*value);
                        score_width = font.width(&text);
                        formatted = Some(text);
                    }
                    max_score = max_score.max(if score_width > 0 { spacer + score_width } else { 0 });
                }
            }
            entries.push((name, score, formatted, score_width));
        }
        if !self.health.is_empty() {
            self.health.retain(|id, _| players.iter().any(|p| p.uuid == *id));
        }
        let slots = players.len() as i32;
        let (mut rows, mut cols) = (slots, 1);
        while rows > 20 {
            cols += 1;
            rows = (slots + cols - 1) / cols;
        }
        let head = if i.online_mode { 9 } else { 0 };
        let width_for_score = i.objective.map_or(0, |o| if o.hearts { 90 } else { max_score });
        let slot_width = (cols * (head + max_name + width_for_score + 13)).min(screen_width - 50) / cols;
        let xxo = screen_width / 2 - (slot_width * cols + (cols - 1) * 5) / 2;
        let mut yyo = 10;
        let mut max_line = slot_width * cols + (cols - 1) * 5;
        let split = |t: &Option<Text>| t.as_ref().map(|t| split_lines(font, t, screen_width - 50, &Style::default()).into_iter().map(|l| l.0).collect::<Vec<Text>>());
        let (header, footer) = (split(&self.header), split(&self.footer));
        for line in header.iter().chain(footer.iter()).flatten() {
            max_line = max_line.max(font.width(line));
        }
        let band = |gui: &mut Gui, y0: i32, y1: i32| gui.fill(screen_width / 2 - max_line / 2 - 1, y0, screen_width / 2 + max_line / 2 + 1, y1, 0x8000_0000);
        let lines = |gui: &mut Gui, lines: &[Text], yyo: &mut i32| {
            for line in lines {
                gui.text(line.clone(), screen_width / 2 - font.width(line) / 2, *yyo, WHITE, true);
                *yyo += 9;
            }
        };
        if let Some(header) = &header {
            band(gui, yyo - 1, yyo + header.len() as i32 * 9);
            lines(gui, header, &mut yyo);
            yyo += 1;
        }
        band(gui, yyo - 1, yyo + rows * 9);
        for (n, (p, (name, score, formatted, score_width))) in players.iter().zip(entries).enumerate() {
            let n = n as i32;
            let mut xo = xxo + (n / rows) * slot_width + (n / rows) * 5;
            let yo = yyo + (n % rows) * 9;
            gui.fill(xo, yo, xo + slot_width, yo + 8, i.background);
            if i.online_mode {
                face(gui, &p.skin, xo, yo, p.show_hat, p.upside_down);
                xo += 9;
            }
            let spectator = p.game_mode == SPECTATOR;
            gui.text(name, xo, yo, if spectator { 0x90FF_FFFF } else { WHITE }, true);
            if let Some(objective) = i.objective.filter(|_| !spectator) {
                let left = xo + max_name + 1;
                let right = left + width_for_score;
                if right - left > 5 {
                    if objective.hearts {
                        let health = self.health.entry(p.uuid).or_insert_with(|| HealthState::new(score));
                        health.update(score, i.tick);
                        tab_hearts::hearts(gui, font, i.lang, health, i.tick, yo, left, right, score);
                    } else if let Some(text) = formatted {
                        gui.text(text, right - score_width, yo, WHITE, true);
                    }
                }
            }
            gui.sprite(ping_sprite(p.latency), xo - head + slot_width - 11, yo, 10, 8);
        }
        if let Some(footer) = &footer {
            yyo += rows * 9 + 1;
            band(gui, yyo - 1, yyo + footer.len() as i32 * 9);
            lines(gui, footer, &mut yyo);
        }
    }
}

/// `PlayerFaceExtractor.extractRenderState` at size 8: the face, then the hat layer, flipped for
/// upside-down players.
fn face(gui: &mut Gui, skin: &str, x: i32, y: i32, hat: bool, flip: bool) {
    let (v, height) = if flip { (16.0, -8) } else { (8.0, 8) };
    for u in if hat { &[8.0, 40.0][..] } else { &[8.0][..] } {
        gui.draws.push(GuiDraw::Blit {
            texture: skin.to_owned().into(),
            blend: Blend::Textured,
            x,
            y,
            w: 8,
            h: 8,
            uv: [*u, v],
            src: [8, height],
            tex: [64, 64],
            color: WHITE,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn player(uuid: u128, name: &str) -> TabPlayer {
        TabPlayer { uuid, name: name.into(), game_mode: 0, latency: 0, display_name: None, list_order: 0, show_hat: true, skin: default_skin(uuid), upside_down: false }
    }

    #[test]
    fn order_ping_and_skins() {
        let mut a = player(1, "bob");
        let mut b = player(2, "Alice");
        a.game_mode = SPECTATOR;
        b.list_order = -1;
        let players = [a, b, player(3, "carl")];
        let none = |_: &str| -> Option<&Team> { None };
        let no_score = |_: &str| None;
        let i = TabInputs { players: &players, team: &none, score: &no_score, objective: None, online_mode: true, tick: 0, background: 0, lang: &|_| None };
        let names: Vec<&str> = TabList::sorted(&i).iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["carl", "bob", "Alice"]);
        assert_eq!(ping_sprite(149), "icon/ping_5");
        assert_eq!(default_skin(0), "entity/player/slim/alex");
        assert_eq!(TabList::name_for_display(&players[0], &i).spans[0].style.italic, Some(true));
    }
}
