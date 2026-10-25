//! The `hud` harness's sidebar and camera overlay kinds.

use std::collections::HashMap;

use serde_json::{Map, Value, json};

use acacia_ui::java::gui::Gui;
use acacia_ui::java::hud::overlays::{self, CameraOverlays};
use acacia_ui::java::hud::sidebar::{self, NumberFormat, ScoreEntry, Sidebar, Team};
use acacia_ui::java::text::{Style, named_color};

use super::kinds::{background, float, int};
use crate::Assets;
use crate::lang::comp;

/// A `Style` from its JSON (`Style.Serializer`).
fn style(assets: &Assets, v: &Value) -> Style {
    let mut o = v.as_object().cloned().unwrap_or_default();
    o.insert("text".into(), json!("x"));
    comp(assets, &Value::Object(o)).spans.first().map(|s| s.style.clone()).unwrap_or_default()
}

/// `null`, `{type: blank}`, `{type: fixed, value}` or `{type: styled, style}`.
pub fn number_format(assets: &Assets, v: &Value) -> Option<NumberFormat> {
    Some(match v["type"].as_str()? {
        "blank" => NumberFormat::Blank,
        "fixed" => NumberFormat::Fixed(comp(assets, &v["value"])),
        "styled" => NumberFormat::Styled(style(assets, &v["style"])),
        other => panic!("unknown number format {other}"),
    })
}

pub fn sidebar(c: &Value, assets: &Assets, gui: &mut Gui) {
    let list = |k: &str| c[k].as_array().cloned().unwrap_or_default();
    let scores: Vec<ScoreEntry> = list("scores")
        .iter()
        .map(|s| ScoreEntry {
            owner: s["owner"].as_str().unwrap_or_default().to_owned(),
            value: int(s, "value", 0),
            display: (!s["display"].is_null()).then(|| comp(assets, &s["display"])),
            format: number_format(assets, &s["format"]),
        })
        .collect();
    let mut teams = Vec::new();
    // Scoreboard.addPlayerToTeam moves a player out of its previous team.
    let mut member_of: HashMap<String, usize> = HashMap::new();
    for (i, t) in list("teams").iter().enumerate() {
        teams.push(Team {
            name: t["name"].as_str().unwrap_or_default().to_owned(),
            prefix: comp(assets, &t["prefix"]),
            suffix: comp(assets, &t["suffix"]),
            color: t["color"].as_str().and_then(named_color),
        });
        for m in t["members"].as_array().into_iter().flatten().filter_map(Value::as_str) {
            member_of.insert(m.to_owned(), i);
        }
    }
    let team = |owner: &str| member_of.get(owner).map(|&i| &teams[i]);
    let o = &c["objective"];
    let title = comp(assets, &o["display"]);
    let format = number_format(assets, &o["format"]);
    let s = Sidebar { title: &title, format: format.as_ref(), scores: &scores, team: &team, background: background(c, 0.3), header: background(c, 0.4) };
    sidebar::sidebar(gui, assets.font.as_ref(), &s);
}

/// `WorldBorder.getDistanceToBorder` of a static border from the camera; `None` without one.
fn border_distance(c: &Value) -> Option<(f32, &Value)> {
    let b = c.get("border").filter(|b| !b.is_null())?;
    let max = 29_999_984.0;
    let half = b["size"].as_f64().unwrap_or(0.0) / 2.0;
    let (cx, cz) = (b["centerX"].as_f64().unwrap_or(0.0), b["centerZ"].as_f64().unwrap_or(0.0));
    let (min_x, max_x) = ((cx - half).clamp(-max, max), (cx + half).clamp(-max, max));
    let (min_z, max_z) = ((cz - half).clamp(-max, max), (cz + half).clamp(-max, max));
    // The camera stand-in's position is a float widened to double.
    let (x, z) = (f64::from(float(c, "camX", 0.0)), f64::from(float(c, "camZ", 0.0)));
    Some(((x - min_x).min(max_x - x).min(z - min_z).min(max_z - z) as f32, b))
}

/// `Hud.extractCameraOverlays` then `extractSleepOverlay`; reads `scopeScale` back.
pub fn overlays(c: &Value, gui: &mut Gui, state: &mut Map<String, Value>) {
    let partial = float(c, "partial", 0.0);
    let warning = border_distance(c).map_or(0.0, |(d, b)| overlays::border_warning(d, int(b, "warningBlocks", 5), int(b, "warningTime", 15), 0.0, 0.0));
    let ticks_frozen = int(c, "ticksFrozen", 0);
    let inputs = CameraOverlays {
        vignette: c["vignette"].as_bool().unwrap_or(false).then(|| (float(c, "vignetteBrightness", 1.0), warning)),
        first_person: c["firstPerson"].as_bool().unwrap_or(false),
        scoping: c["scoping"].as_bool().unwrap_or(false),
        game_time_delta: partial,
        equipment: if c["head"].as_str() == Some("carved_pumpkin") { vec!["misc/pumpkinblur".to_owned()] } else { Vec::new() },
        ticks_frozen,
        percent_frozen: ticks_frozen.min(140) as f32 / 140.0,
        partial,
        portal: [c["portal"][0].as_f64().unwrap_or(0.0) as f32, c["portal"][1].as_f64().unwrap_or(0.0) as f32],
        nausea: float(c, "nausea", 0.0),
        screen_effect_scale: float(c, "screenEffectScale", 1.0),
    };
    let mut scope = float(c, "scopeScale", 0.5);
    overlays::camera_overlays(gui, &inputs, &mut scope);
    overlays::sleep_overlay(gui, int(c, "sleepTimer", 0));
    state.insert("scopeScale".into(), json!(scope));
}
