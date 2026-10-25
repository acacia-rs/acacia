//! `Hud.extractRenderState` and `extractDeferredSubtitles`, with the part of `Hud.tick` that feeds
//! them: which pieces draw, in what order and under which conditions. [`HudInputs`] is what the
//! game reads from `Minecraft` for one frame, as plain data; [`Hud`] is what `Hud` keeps between
//! frames. Not ported, so never drawn: the demo timer, the locator bar, the spectator menu.

use std::collections::HashMap;

use super::bars::{self, BossBar};
use super::effects::{self, EffectIcon};
use super::health::{Health, HealthHud};
use super::hotbar::{self, Hotbar};
use super::overlays::{self, CameraOverlays};
use super::sidebar::{self, NumberFormat, ScoreEntry, Sidebar, Team};
use super::subtitles::{SubtitleFrame, SubtitleOverlay};
use super::tab_list::{self, ListObjective, TabInputs, TabList, TabPlayer};
use super::titles::{HeldName, Titles, ToolHighlight, background_color};
use crate::java::chat::component::Chat;
use crate::java::gui::{Gui, Metrics};
use crate::java::text::{Lang, Text};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GameType {
    #[default]
    Survival,
    Creative,
    Adventure,
    Spectator,
}

impl GameType {
    /// `canHurtPlayer`, which is also `hasExperience`.
    pub fn survival_like(self) -> bool {
        matches!(self, GameType::Survival | GameType::Adventure)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AttackIndicator {
    Off,
    #[default]
    Crosshair,
    Hotbar,
}

/// The open screen, as the HUD asks about it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ScreenInfo {
    /// A `LevelLoadingScreen`: no HUD at all.
    pub level_loading: bool,
    /// `isInGameUi`: containers, books, signs and block editors.
    pub in_game_ui: bool,
    /// `showsActiveEffects`; see [`container_shows_effects`].
    pub shows_effects: bool,
}

/// `EffectsInInventory.canSeeEffects` for a centred container `image_width` wide.
pub fn container_shows_effects(screen_width: i32, image_width: i32) -> bool {
    let left = (screen_width - image_width) / 2;
    screen_width - (left + image_width + 2) >= 32
}

/// An objective in a sidebar display slot.
#[derive(Clone, Debug, PartialEq)]
pub struct SidebarObjective {
    pub title: Text,
    pub format: Option<NumberFormat>,
    /// `listPlayerScores`.
    pub scores: Vec<ScoreEntry>,
}

/// What `PlayerTabOverlay` reads besides its own state.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TabFrame {
    /// `options.keyPlayerList.isDown`.
    pub key_down: bool,
    pub local_server: bool,
    /// `getListedOnlinePlayers`.
    pub players: Vec<TabPlayer>,
    /// The objective in the `LIST` slot.
    pub objective: Option<ListObjective>,
    /// `getPlayerScoreInfo` by player name.
    pub scores: HashMap<String, (i32, Option<NumberFormat>)>,
    pub online_mode: bool,
}

/// A ridden living vehicle that shows its health.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vehicle {
    pub max_health: f32,
    pub health: f32,
}

/// What one frame of `Hud.extractRenderState` reads.
#[derive(Clone, Debug, Default)]
pub struct HudInputs {
    pub width: i32,
    pub height: i32,
    /// `Hud.isHidden` (F1).
    pub hidden: bool,
    pub screen: Option<ScreenInfo>,
    pub game_mode: GameType,
    pub first_person: bool,
    /// `getGameTimeDeltaPartialTick(false)` and `getGameTimeDeltaTicks`.
    pub partial: f32,
    pub game_time_delta: f32,
    /// `Util.getMillis()`.
    pub millis: i64,
    /// `canRenderCrosshairForSpectator(hitResult)`.
    pub spectator_crosshair: bool,
    /// The `3d_crosshair` debug entry is enabled (it is while F3 shows).
    pub crosshair_3d: bool,
    pub attack_indicator: AttackIndicator,
    /// `getAttackStrengthScale(0)`, `getCurrentItemAttackStrengthDelay`, and whether the
    /// `crosshairPickEntity` is a living one that is alive (`None`: no entity).
    pub attack_scale: f32,
    pub attack_delay: f32,
    pub attack_target: Option<bool>,
    pub camera: CameraOverlays,
    /// Its `attack` is filled from `attack_indicator`.
    pub hotbar: Hotbar,
    /// Its `tick`, `millis` and `vehicle_hearts` are filled from the frame.
    pub health: Health,
    pub vehicle: Option<Vehicle>,
    /// `jumpableVehicle()`: its jump cooldown and the player's `getJumpRidingScale`.
    pub jump: Option<(i32, f32)>,
    pub xp_progress: f32,
    pub xp_level: i32,
    /// `getActiveEffects` in its order.
    pub effects: Vec<EffectIcon>,
    pub demo: bool,
    pub bosses: Vec<BossBar>,
    pub sleep_timer: i32,
    /// The objective in the player's team colour slot, and in the `SIDEBAR` slot.
    pub team_sidebar: Option<SidebarObjective>,
    pub sidebar: Option<SidebarObjective>,
    /// `getPlayersTeam` by player name.
    pub teams: HashMap<String, Team>,
    /// `backgroundForChatOnly` and `textBackgroundOpacity`.
    pub chat_only: bool,
    pub text_opacity: f32,
    /// `ChatComponent.isChatFocused`: the chat screen is open.
    pub chat_focused: bool,
    pub tab: TabFrame,
    /// Its `background` is filled from the text background options.
    pub subtitles: SubtitleFrame,
}

/// What `Hud` keeps between frames.
#[derive(Clone, Debug, Default)]
pub struct Hud {
    pub tick_count: i32,
    pub health: HealthHud,
    pub scope_scale: f32,
    pub titles: Titles,
    pub highlight: ToolHighlight,
    pub chat: Chat,
    pub tab: TabList,
    pub subtitles: SubtitleOverlay,
    /// `deferredSubtitles`: the frame and time it was deferred with.
    deferred: Option<(SubtitleFrame, i64)>,
}

impl Hud {
    /// The parts of `Hud.tick` the HUD reads: `selected` is the selected stack,
    /// `display_time` `notificationDisplayTime`.
    pub fn tick(&mut self, selected: Option<HeldName>, display_time: f64) {
        self.titles.tick();
        self.tick_count += 1;
        self.highlight.tick(selected, display_time);
    }

    /// `Hud.extractRenderState`.
    pub fn extract(&mut self, gui: &mut Gui, font: &dyn Metrics, lang: Lang, i: &HudInputs) {
        if i.screen.is_some_and(|s| s.level_loading) {
            return;
        }
        let in_game_ui = i.screen.is_some_and(|s| s.in_game_ui);
        if !i.hidden {
            overlays::camera_overlays(gui, &i.camera, &mut self.scope_scale);
            self.crosshair(gui, i);
            gui.next_stratum();
            self.hotbar_and_decorations(gui, font, i);
            if !i.effects.is_empty() && !i.screen.is_some_and(|s| s.shows_effects) {
                effects::effects(gui, i.demo, &i.effects);
            }
            bars::boss_overlay(gui, font, &i.bosses);
        }
        overlays::sleep_overlay(gui, i.sleep_timer);
        if !i.hidden {
            self.sidebar(gui, font, i);
            let background = background_color(i.chat_only, i.text_opacity, 0.0);
            self.titles.overlay_message(gui, font, i.partial, background);
            self.titles.title(gui, font, i.partial, background);
            if !i.chat_focused {
                gui.next_stratum();
                self.chat.extract(gui, self.tick_count, false);
            }
            self.tab_list(gui, font, lang, i);
            self.subtitle_overlay(gui, font, i, i.screen.is_none() || in_game_ui);
        } else if in_game_ui {
            self.subtitle_overlay(gui, font, i, true);
        }
    }

    /// `Hud.extractDeferredSubtitles`: after the screen, toasts and debug overlay.
    pub fn extract_deferred_subtitles(&mut self, gui: &mut Gui, font: &dyn Metrics) {
        if let Some((frame, now)) = self.deferred.take() {
            self.subtitles.extract(gui, font, &frame, now);
        }
    }

    fn crosshair(&self, gui: &mut Gui, i: &HudInputs) {
        let spectator = i.game_mode == GameType::Spectator;
        if !i.first_person || spectator && !i.spectator_crosshair || i.crosshair_3d {
            return;
        }
        let attack = (i.attack_indicator == AttackIndicator::Crosshair).then_some(i.attack_scale);
        let full = attack.is_some_and(|a| hotbar::full_attack_indicator(a, i.attack_delay, i.attack_target));
        hotbar::crosshair(gui, attack, full);
    }

    /// `extractHotbarAndDecorations`, with the contextual bar as `nextContextualInfoState` picks
    /// it (no waypoints, so never the locator bar).
    fn hotbar_and_decorations(&mut self, gui: &mut Gui, font: &dyn Metrics, i: &HudInputs) {
        let spectator = i.game_mode == GameType::Spectator;
        let survival = i.game_mode.survival_like();
        if !spectator {
            let attack = (i.attack_indicator == AttackIndicator::Hotbar).then_some(i.attack_scale);
            hotbar::hotbar(gui, &Hotbar { attack, ..i.hotbar.clone() });
        }
        let vehicle_hearts = i.vehicle.map_or(0, |v| hotbar::vehicle_max_hearts(v.max_health));
        if survival {
            let health = Health { tick: self.tick_count, millis: i.millis, vehicle_hearts, ..i.health.clone() };
            self.health.extract(gui, &health);
        }
        if let Some(v) = i.vehicle.filter(|_| vehicle_hearts != 0) {
            hotbar::vehicle_health(gui, vehicle_hearts, v.health);
        }
        if let Some((cooldown, scale)) = i.jump {
            bars::jump_bar(gui, cooldown, scale);
        }
        let level = if survival { i.xp_level } else { 0 };
        hotbar::experience(gui, font, i.xp_progress, level, survival && i.jump.is_none());
        if !spectator {
            self.highlight.draw(gui, font, survival, background_color(i.chat_only, i.text_opacity, 0.0));
        }
    }

    /// `extractScoreboardSidebar`: the team colour slot's objective before the sidebar's.
    fn sidebar(&self, gui: &mut Gui, font: &dyn Metrics, i: &HudInputs) {
        let Some(o) = i.team_sidebar.as_ref().or(i.sidebar.as_ref()) else { return };
        gui.next_stratum();
        let team = |name: &str| i.teams.get(name);
        let s = Sidebar {
            title: &o.title,
            format: o.format.as_ref(),
            scores: &o.scores,
            team: &team,
            background: background_color(i.chat_only, i.text_opacity, 0.3),
            header: background_color(i.chat_only, i.text_opacity, 0.4),
        };
        sidebar::sidebar(gui, font, &s);
    }

    /// `extractTabList`.
    fn tab_list(&mut self, gui: &mut Gui, font: &dyn Metrics, lang: Lang, i: &HudInputs) {
        let t = &i.tab;
        let visible = t.key_down && !(t.local_server && t.players.len() <= 1 && t.objective.is_none());
        self.tab.set_visible(visible);
        if !visible {
            return;
        }
        gui.next_stratum();
        let team = |name: &str| i.teams.get(name);
        let score = |name: &str| t.scores.get(name).cloned();
        let inputs = TabInputs {
            players: &t.players,
            team: &team,
            score: &score,
            objective: t.objective.as_ref(),
            online_mode: t.online_mode,
            tick: i64::from(self.tick_count),
            background: tab_list::row_background(i.chat_only, i.text_opacity),
            lang,
        };
        let width = gui.width;
        self.tab.extract(gui, font, &inputs, width);
    }

    /// `extractSubtitleOverlay`.
    fn subtitle_overlay(&mut self, gui: &mut Gui, font: &dyn Metrics, i: &HudInputs, defer: bool) {
        let frame = SubtitleFrame { background: background_color(i.chat_only, i.text_opacity, 0.8), ..i.subtitles };
        if defer {
            self.deferred = Some((frame, i.millis));
        } else {
            self.deferred = None;
            self.subtitles.extract(gui, font, &frame, i.millis);
        }
    }
}

#[cfg(test)]
#[path = "frame_tests.rs"]
mod tests;
