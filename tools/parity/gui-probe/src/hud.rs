//! The `hud` harness's kinds; the case schema is `tools/parity/harnesses/hud/cases.py`'s.

use serde_json::{Map, Value};

use acacia_ui::java::gui::{Gui, GuiDraw};
use acacia_ui::java::hud::health::{Health, HealthHud, HeartKind};
use acacia_ui::java::hud::hotbar::{self, Hotbar};

use crate::Assets;

mod kinds;
mod scene;

pub fn replay(c: &Value, assets: &Assets, state: &mut Map<String, Value>) -> Vec<GuiDraw> {
    let mut gui = kinds::screen(c);
    match c["kind"].as_str().unwrap_or_default() {
        "decorations" => return decorations(c, assets, state),
        "chat" => return crate::chat::replay_hud(c, assets, state),
        "effects" => kinds::effects(c, &mut gui),
        "title" => kinds::title(c, assets, &mut gui),
        "selected" => kinds::selected(c, assets, &mut gui),
        "boss" => kinds::boss(c, assets, &mut gui),
        "jump" => kinds::jump(c, &mut gui),
        "sidebar" => scene::sidebar(c, assets, &mut gui),
        "decor" => kinds::decor(c, assets, &mut gui),
        "overlays" => scene::overlays(c, &mut gui, state),
        // A kind not ported yet draws nothing, so it shows as a divergence.
        _ => {}
    }
    gui.draws
}

fn decorations(c: &Value, assets: &Assets, _: &mut Map<String, Value>) -> Vec<GuiDraw> {
    let int = |k: &str, d: i64| c[k].as_i64().unwrap_or(d) as i32;
    let float = |k: &str, d: f64| c[k].as_f64().unwrap_or(d) as f32;
    let mut gui = Gui::new(int("width", 0), int("height", 0));
    let mode = c["gameMode"].as_str().unwrap_or("survival");
    let survival = mode == "survival" || mode == "adventure";
    let indicator = c["attackIndicator"].as_str().unwrap_or("OFF");
    let partial = float("partial", 0.0);
    let effects: Vec<&str> = c["effects"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
    let has = |e: &str| effects.contains(&e);
    if c["crosshair"].as_bool().unwrap_or(false) && mode != "spectator" {
        let attack = (indicator == "CROSSHAIR").then(|| float("attackScale", 1.0));
        let full = attack.is_some_and(|a| hotbar::full_attack_indicator(a, float("attackDelay", 5.0), c["target"].as_bool()));
        hotbar::crosshair(&mut gui, attack, full);
    }
    if mode != "spectator" {
        let pop = |v: &Value| v.as_f64().map(|p| p as f32 - partial);
        let bar = Hotbar {
            selected: int("selected", 0),
            right_handed: c["rightHanded"].as_bool().unwrap_or(true),
            slots: std::array::from_fn(|i| pop(&c["slots"][i])),
            offhand: pop(&c["offhand"]),
            attack: (indicator == "HOTBAR").then(|| float("attackScale", 1.0)),
        };
        hotbar::hotbar(&mut gui, &bar);
    }
    let vehicle_hearts = if c.get("vehicleMaxHealth").is_some() { hotbar::vehicle_max_hearts(float("vehicleMaxHealth", 0.0)) } else { 0 };
    if survival {
        let millis = 1_000_000_000i64;
        let mut hud = HealthHud {
            last_health: int("lastHealth", 0),
            display_health: int("displayHealth", 0),
            last_health_time: millis - i64::from(int("lastHealthAgo", 0)),
            health_blink_time: i64::from(int("healthBlinkTime", 0)),
        };
        let kind = if has("minecraft:poison") {
            HeartKind::Poisoned
        } else if has("minecraft:wither") {
            HeartKind::Withered
        } else if c["frozen"].as_bool().unwrap_or(false) {
            HeartKind::Frozen
        } else {
            HeartKind::Normal
        };
        let state = Health {
            tick: int("tick", 0),
            millis,
            health: float("health", 20.0),
            max_health: float("maxHealth", 20.0),
            absorption: float("absorption", 0.0),
            armor: int("armor", 0),
            food: int("food", 20),
            saturation: float("saturation", 5.0),
            kind,
            regeneration: has("minecraft:regeneration"),
            hunger: has("minecraft:hunger"),
            hardcore: c["hardcore"].as_bool().unwrap_or(false),
            damage_cooldown: int("damageCooldown", 0) > 0,
            air: int("air", 300),
            max_air: int("maxAir", 300),
            under_water: c["underWater"].as_bool().unwrap_or(false),
            vehicle_hearts,
        };
        hud.extract(&mut gui, &state);
    }
    if vehicle_hearts > 0 {
        hotbar::vehicle_health(&mut gui, vehicle_hearts, float("vehicleHealth", 0.0));
    }
    let level = if survival { int("xpLevel", 0) } else { 0 };
    hotbar::experience(&mut gui, assets.font.as_ref(), float("xpProgress", 0.0), level, c["experienceBar"].as_bool().unwrap_or(false));
    gui.draws
}
