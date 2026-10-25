//! The Java look's HUD as the game draws it: `acacia_ui::java::hud` layouts, painted with the
//! pack's sprites in the game's order (`acacia_ui::java::paint`).

use std::time::Instant;

use acacia_bot::proto::types::GameMode;
use acacia_ui::java::gui::{Gui, GuiDraw, Metrics};
use acacia_ui::java::hud::health::{Health, HealthHud, HeartKind};
use acacia_ui::java::hud::hotbar::{self, Hotbar, OFFHAND_KEY};
use acacia_ui::java::paint::assets::Images;
use acacia_ui::java::paint::blit::Scaling;
use acacia_ui::java::paint::{self, Glyphs, Source};
use acacia_ui::java::text::Text;
use acacia_ui::{DrawList, Quad, Sprite};

use super::Skin;
use crate::control::Stack;
use crate::me::Me;

// Bedrock's effect ids.
const REGENERATION: i32 = 10;
const HUNGER: i32 = 17;
const POISON: i32 = 19;
const WITHER: i32 = 20;

pub struct JavaHud {
    images: Images,
    health: HealthHud,
    started: Instant,
}

impl JavaHud {
    pub fn new(root: &std::path::Path) -> JavaHud {
        JavaHud { images: Images::new(root), health: HealthHud::default(), started: Instant::now() }
    }

    /// Whether the pack has the HUD's sprites.
    pub fn usable(&mut self, skin: &mut Skin) -> bool {
        self.images.sprite(&mut skin.theme.atlas, "hud/hotbar").is_some()
    }

    /// `Hud.extractRenderState`'s crosshair, hotbar, health, food, air and experience for `me`.
    pub fn extract(&mut self, me: &Me, size: [i32; 2], crosshair: bool, font: &dyn Metrics, now: Instant) -> Gui {
        let mut gui = Gui::new(size[0], size[1]);
        let spectator = matches!(me.game_mode, GameMode::Spectator | GameMode::SurvivalSpectator | GameMode::CreativeSpectator);
        let survival = matches!(me.game_mode, GameMode::Survival | GameMode::Adventure);
        if crosshair && !spectator {
            hotbar::crosshair(&mut gui, None, false);
        }
        if !spectator {
            let bar = Hotbar {
                selected: i32::from(me.hotbar % 9),
                right_handed: true,
                slots: std::array::from_fn(|i| me.items[i].as_ref().map(|_| 0.0)),
                offhand: me.offhand.as_ref().map(|_| 0.0),
                attack: None,
            };
            hotbar::hotbar(&mut gui, &bar);
        }
        if survival {
            let millis = now.saturating_duration_since(self.started).as_millis() as i64;
            let has = |id| me.effects.iter().any(|e| e.0 == id);
            let kind = if has(POISON) {
                HeartKind::Poisoned
            } else if has(WITHER) {
                HeartKind::Withered
            } else {
                HeartKind::Normal
            };
            let (air, max_air) = me.air.map_or((300, 300), |(a, m)| (a as i32, m as i32));
            let state = Health {
                tick: (millis / 50) as i32,
                millis,
                health: me.health,
                max_health: me.max_health,
                absorption: 0.0,
                armor: me.armor as i32,
                food: me.food as i32,
                saturation: 0.0,
                kind,
                regeneration: has(REGENERATION),
                hunger: has(HUNGER),
                hardcore: false,
                damage_cooldown: false,
                air,
                max_air,
                under_water: me.air.is_some(),
                vehicle_hearts: 0,
            };
            self.health.extract(&mut gui, &state);
        }
        hotbar::experience(&mut gui, font, me.xp_progress, if survival { me.xp_level } else { 0 }, survival);
        gui
    }

    /// `gui`'s calls as quads into `list`, the hotbar's items from `me`.
    pub fn paint(&mut self, skin: &mut Skin, me: &Me, gui: &Gui, list: &mut DrawList) {
        let mut src = Src { skin, images: &mut self.images, me };
        let painted = paint::paint(&gui.draws, [gui.width, gui.height], list.scale, &mut src);
        list.quads.extend(painted.quads);
    }
}

struct Src<'a> {
    skin: &'a mut Skin,
    images: &'a mut Images,
    me: &'a Me,
}

impl Src<'_> {
    fn stack(&self, key: i32) -> Option<&Stack> {
        match key {
            OFFHAND_KEY => self.me.offhand.as_ref(),
            k => self.me.items.get(k as usize)?.as_ref(),
        }
    }
}

impl Source for Src<'_> {
    fn sprite(&mut self, id: &str) -> Option<(Sprite, Scaling)> {
        self.images.sprite(&mut self.skin.theme.atlas, id)
    }

    fn block_sprite(&mut self, id: &str) -> Option<Sprite> {
        self.images.block_sprite(&mut self.skin.theme.atlas, id)
    }

    fn texture(&mut self, path: &str) -> Option<Sprite> {
        self.images.texture(&mut self.skin.theme.atlas, path)
    }

    fn white(&mut self) -> Sprite {
        self.skin.theme.atlas.white()
    }

    // TODO: the Java font's glyph layout (java/font) in place of the ASCII page, styles and all.
    fn text(&mut self, text: &Text, x: i32, y: i32, color: u32, shadow: bool) -> Glyphs {
        let Some(font) = &self.skin.theme.font else { return Glyphs::default() };
        let mut list = DrawList::new(1.0);
        let plain = text.resolve_legacy().plain();
        let width = font.draw(&mut list, &plain, x as f32, y as f32, color & 0xFF_FFFF, (color >> 24) as f32 / 255.0, shadow);
        let edge = if shadow { 1.0 } else { 0.0 };
        Glyphs { quads: list.quads, bounds: Some([x as f32, y as f32, x as f32 + width + edge, y as f32 + 8.0 + edge]) }
    }

    fn item(&mut self, key: i32, x: i32, y: i32, _: i32) -> Vec<Quad> {
        let Some(stack) = self.stack(key).cloned() else { return Vec::new() };
        let Some(item) = self.skin.item(&stack) else { return Vec::new() };
        let mut list = DrawList::new(1.0);
        list.icon(item.icon, [x as f32, y as f32, x as f32 + 16.0, y as f32 + 16.0], item.glint);
        list.quads
    }

    /// `itemDecorations`: the count (bars and cooldowns are not known here).
    fn decorations(&mut self, key: i32, x: i32, y: i32, count: Option<&str>) -> Vec<GuiDraw> {
        let Some(stack) = self.stack(key) else { return Vec::new() };
        let Some(font) = &self.skin.theme.font else { return Vec::new() };
        if stack.count == 1 && count.is_none() {
            return Vec::new();
        }
        let amount = count.map_or_else(|| stack.count.to_string(), str::to_owned);
        let width = Metrics::width(font, &Text::literal(amount.as_str()));
        vec![GuiDraw::Text { text: Text::literal(amount), x: x + 19 - 2 - width, y: y + 6 + 3, color: 0xFFFF_FFFF, shadow: true }]
    }
}
