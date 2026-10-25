//! Viewer and bot state as `Hud.extractRenderState` reads it. What the viewer does not know takes
//! the value Java has when the thing is absent: no vehicle, jump bar, absorption, hardcore, damage
//! blink, freezing, portal, sleep, spyglass, crosshair entity or attack cooldown (full strength),
//! no teams, team sidebar, ping (0), list objective or tab header and footer; first person; the
//! camera at full light (the vignette at its daylight strength, none); subtitles off (the option's
//! default); options at their defaults.

use std::collections::HashMap;

use acacia_bot::proto::types::GameMode;
use acacia_ui::effects::{Active, EFFECTS};
use acacia_ui::java::hud::bars::BossBar;
use acacia_ui::java::hud::effects::EffectIcon;
use acacia_ui::inventory::Layout;
use acacia_ui::java::hud::frame::{AttackIndicator, GameType, HudInputs, ScreenInfo, SidebarObjective, TabFrame, container_shows_effects};
use acacia_ui::java::hud::health::{Health, HeartKind};
use acacia_ui::java::hud::hotbar::Hotbar;
use acacia_ui::java::hud::overlays::CameraOverlays;
use acacia_ui::java::hud::sidebar::ScoreEntry;
use acacia_ui::java::hud::subtitles::SubtitleFrame;
use acacia_ui::java::hud::tab_list::{TabPlayer, default_skin};
use acacia_ui::java::hud::titles::HeldName;
use acacia_ui::java::text::Text;
use acacia_ui::lang::Lang;
use acacia_ui::sidebar::Sidebar;

use super::HudView;
use crate::me::Me;

// Bedrock's effect ids.
const NAUSEA: i32 = 9;
const REGENERATION: i32 = 10;
const HUNGER: i32 = 17;
const POISON: i32 = 19;
const WITHER: i32 = 20;
const FATAL_POISON: i32 = 25;

/// `Rarity.color()` of `COMMON` and of `RARE`, which enchanting makes a common stack.
const COMMON: u32 = 0xFF_FFFF;
const RARE: u32 = 0x55_FFFF;

/// One frame's inputs: `millis` is `Util.getMillis()`, `delta` the game ticks since the last frame.
pub fn inputs(v: &HudView, size: [i32; 2], millis: i64, delta: f32) -> HudInputs {
    let me = v.me;
    let partial = (millis % 50) as f32 / 50.0;
    HudInputs {
        width: size[0],
        height: size[1],
        hidden: false,
        screen: v.screen,
        game_mode: game_type(me.game_mode),
        first_person: true,
        partial,
        game_time_delta: delta,
        millis,
        spectator_crosshair: false,
        crosshair_3d: v.debug,
        attack_indicator: AttackIndicator::Crosshair,
        attack_scale: 1.0,
        attack_delay: 0.0,
        attack_target: None,
        camera: camera(me, partial, delta),
        hotbar: Hotbar {
            selected: i32::from(me.hotbar % 9),
            right_handed: true,
            slots: std::array::from_fn(|i| me.items[i].as_ref().map(|_| 0.0)),
            offhand: me.offhand.as_ref().map(|_| 0.0),
            attack: None,
        },
        health: health(me),
        vehicle: None,
        jump: None,
        xp_progress: me.xp_progress,
        xp_level: me.xp_level,
        effects: v.effects.iter().filter_map(effect_icon).collect(),
        demo: false,
        bosses: me.bosses.iter().map(|(title, progress, color)| BossBar { name: Text::literal(v.lang.translate(title, &[])), progress: *progress, color: *color, overlay: 0 }).collect(),
        sleep_timer: 0,
        team_sidebar: None,
        sidebar: v.sidebar.map(sidebar),
        teams: HashMap::new(),
        chat_only: true,
        text_opacity: 0.5,
        chat_focused: v.chat_open,
        tab: TabFrame { key_down: v.players.is_some(), players: v.players.unwrap_or_default().iter().enumerate().map(|(n, name)| tab_player(n, name)).collect(), ..TabFrame::default() },
        subtitles: SubtitleFrame::default(),
    }
}

/// The open screen as the HUD asks about it: `other` is a menu, form or the chat screen (not in-game
/// UI); `in_game` a book or sign; `container` an inventory screen's layout, whose player and
/// creative screens list the effects themselves where they fit beside it.
pub fn screen_info(container: Option<Layout>, in_game: bool, other: bool, width: i32) -> Option<ScreenInfo> {
    if other {
        return Some(ScreenInfo::default());
    }
    if let Some(layout) = container {
        let shows_effects = matches!(layout, Layout::Player | Layout::Creative) && container_shows_effects(width, layout.width() as i32);
        return Some(ScreenInfo { in_game_ui: true, shows_effects, level_loading: false });
    }
    in_game.then(|| ScreenInfo { in_game_ui: true, ..ScreenInfo::default() })
}

pub fn game_type(mode: GameMode) -> GameType {
    match mode {
        GameMode::Creative | GameMode::CreativeSpectator => GameType::Creative,
        GameMode::Adventure => GameType::Adventure,
        GameMode::Spectator | GameMode::SurvivalSpectator => GameType::Spectator,
        _ => GameType::Survival,
    }
}

/// A Bedrock effect as Java's; `None` for one Java has no icon for.
pub fn effect_icon(a: &Active) -> Option<EffectIcon> {
    let &(_, name, _) = EFFECTS.iter().find(|e| e.0 == a.id)?;
    EffectIcon::vanilla(name, a.ticks, a.ambient, true)
}

fn camera(me: &Me, partial: f32, delta: f32) -> CameraOverlays {
    let pumpkin = me.head.as_deref() == Some("minecraft:carved_pumpkin");
    CameraOverlays {
        vignette: Some((0.0, 0.0)),
        first_person: true,
        game_time_delta: delta,
        equipment: if pumpkin { vec!["misc/pumpkinblur".to_owned()] } else { Vec::new() },
        partial,
        nausea: if me.effects.iter().any(|e| e.0 == NAUSEA) { 1.0 } else { 0.0 },
        screen_effect_scale: 1.0,
        ..CameraOverlays::default()
    }
}

fn health(me: &Me) -> Health {
    let has = |id| me.effects.iter().any(|e| e.0 == id);
    let kind = if has(POISON) || has(FATAL_POISON) {
        HeartKind::Poisoned
    } else if has(WITHER) {
        HeartKind::Withered
    } else {
        HeartKind::Normal
    };
    let (air, max_air) = me.air.map_or((300, 300), |(a, m)| (a as i32, m as i32));
    Health {
        health: me.health,
        max_health: me.max_health,
        armor: me.armor as i32,
        food: me.food as i32,
        saturation: me.saturation,
        kind,
        regeneration: has(REGENERATION),
        hunger: has(HUNGER),
        air,
        max_air,
        under_water: me.air.is_some(),
        ..Health::default()
    }
}

/// The selected stack as `Hud.tick` compares it: its anvil name, else the item's.
pub fn held_name(me: &Me, lang: &Lang) -> Option<HeldName> {
    let stack = me.items[usize::from(me.hotbar % 9)].as_ref()?;
    Some(HeldName {
        item: stack.name.clone(),
        hover_name: Text::literal(stack.title.clone().unwrap_or_else(|| lang.item_name(&stack.name))),
        rarity_color: if stack.enchanted { RARE } else { COMMON },
        custom_name: stack.title.is_some(),
    })
}

/// The viewer's sidebar, as the objective Java sorts and trims itself.
pub fn sidebar(s: &Sidebar) -> SidebarObjective {
    SidebarObjective {
        title: Text::literal(s.title.clone()),
        format: None,
        scores: s.lines.iter().map(|(owner, value)| ScoreEntry { owner: owner.clone(), value: *value, display: None, format: None }).collect(),
    }
}

fn tab_player(n: usize, name: &str) -> TabPlayer {
    let uuid = n as u128;
    TabPlayer { uuid, name: name.to_owned(), game_mode: 0, latency: 0, display_name: None, list_order: 0, show_hat: true, skin: default_skin(uuid), upside_down: false }
}

#[cfg(test)]
#[path = "inputs_tests.rs"]
mod tests;
