//! The `players` harness: replays a list's frames (`tools/parity/harnesses/players/cases.py`)
//! through `TabList` and keeps the last frame's draws.

use std::collections::HashMap;

use serde_json::{Map, Value};

use acacia_ui::java::gui::{Gui, GuiDraw};
use acacia_ui::java::hud::sidebar::{NumberFormat, Team};
use acacia_ui::java::hud::tab_list::{ListObjective, TabInputs, TabList, TabPlayer, default_skin, row_background};
use acacia_ui::java::text::named_color;

use crate::Assets;
use crate::hud::number_format;
use crate::lang::{comp, table};

pub fn replay(c: &Value, assets: &Assets, _: &mut Map<String, Value>) -> Vec<GuiDraw> {
    let list = |v: &Value| v.as_array().cloned().unwrap_or_default();
    let mut teams = Vec::new();
    // Scoreboard.addPlayerToTeam moves a player out of its previous team.
    let mut member_of: HashMap<String, usize> = HashMap::new();
    for (i, t) in list(&c["teams"]).iter().enumerate() {
        teams.push(Team {
            name: t["name"].as_str().unwrap_or_default().to_owned(),
            prefix: comp(assets, &t["prefix"]),
            suffix: comp(assets, &t["suffix"]),
            color: t["color"].as_str().and_then(named_color),
        });
        for m in list(&t["members"]).iter().filter_map(Value::as_str) {
            member_of.insert(m.to_owned(), i);
        }
    }
    let objective = (!c["objective"].is_null())
        .then(|| ListObjective { hearts: c["objective"]["renderType"] == "hearts", format: number_format(assets, &c["objective"]["numberFormat"]) });
    let players: Vec<TabPlayer> = list(&c["players"])
        .iter()
        .map(|p| {
            let uuid = u128::from_str_radix(&p["uuid"].as_str().unwrap_or("0").replace('-', ""), 16).unwrap_or(0);
            let name = p["name"].as_str().unwrap_or_default().to_owned();
            TabPlayer {
                uuid,
                upside_down: p["inLevel"].as_bool().unwrap_or(false) && (name == "Dinnerbone" || name == "Grumm"),
                name,
                game_mode: p["gameMode"].as_i64().unwrap_or(0) as i32,
                latency: p["latency"].as_i64().unwrap_or(0) as i32,
                display_name: Some(p["displayName"].clone()).filter(|d| !d.is_null()),
                list_order: p["listOrder"].as_i64().unwrap_or(0) as i32,
                show_hat: p["showHat"].as_bool().unwrap_or(true),
                skin: default_skin(uuid),
            }
        })
        .collect();
    let lang = table(assets);
    let lang = |k: &str| lang.get(k).cloned();
    let team = |owner: &str| member_of.get(owner).map(|&i| &teams[i]);
    let mut tab = TabList::default();
    let text = |v: &Value| (!v.is_null()).then(|| comp(assets, v));
    (tab.header, tab.footer) = (text(&c["header"]), text(&c["footer"]));
    let width = c["width"].as_i64().unwrap_or(0) as i32;
    let mut scores: HashMap<String, (i32, Option<NumberFormat>)> = HashMap::new();
    let mut gui = Gui::default();
    for f in list(&c["frames"]) {
        if objective.is_some() {
            for (holder, v) in f["scores"].as_object().into_iter().flatten() {
                scores.insert(holder.clone(), (v["value"].as_i64().unwrap_or(0) as i32, number_format(assets, &v["format"])));
            }
            for r in list(&f["reset"]).iter().filter_map(Value::as_str) {
                scores.remove(r);
            }
        }
        let visible = f["visible"].as_bool().unwrap_or(false);
        tab.set_visible(visible);
        gui = Gui::new(width, 240);
        if visible {
            let score = |owner: &str| scores.get(owner).cloned();
            let inputs = TabInputs {
                players: &players,
                team: &team,
                score: &score,
                objective: objective.as_ref(),
                online_mode: c["onlineMode"].as_bool().unwrap_or(false),
                tick: f["tick"].as_i64().unwrap_or(0),
                background: row_background(c["chatOnly"].as_bool().unwrap_or(true), c["opacity"].as_f64().unwrap_or(0.5) as f32),
                lang: &lang,
            };
            tab.extract(&mut gui, assets.font.as_ref(), &inputs, width);
        }
    }
    gui.draws
}
