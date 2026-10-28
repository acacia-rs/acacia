//! `GuiGraphicsExtractor.itemDecorations`: durability bar, cooldown shade and count. Ported from
//! Horizon's `hud_overlays.rs` (MIT).

use crate::java::gui::{Gui, Metrics};
use crate::java::mth::{BLACK, WHITE, ceil, clamp, floor, hsv_to_argb};
use crate::java::tooltip::bundle;
use crate::java::tooltip::game::Game;
use crate::java::tooltip::stack::ItemStack;

/// What `itemDecorations` reads from a non-empty stack.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ItemDecorations {
    pub count: i32,
    /// The text drawn instead of the count.
    pub count_text: Option<String>,
    /// `isBarVisible`: `getBarWidth` and `getBarColor` (RGB).
    pub bar: Option<(i32, u32)>,
    /// `ItemCooldowns.getCooldownPercent`.
    pub cooldown: f32,
}

/// `Item.getBarWidth` and `getBarColor` of a stack with `damage` out of `max_damage`
/// (0 when not damageable or unbreakable); `None` when undamaged.
pub fn damage_bar(damage: i32, max_damage: i32) -> Option<(i32, u32)> {
    if max_damage <= 0 || damage <= 0 {
        return None;
    }
    // Math.round(float): floor(x + 0.5).
    let width = floor(13.0 - damage as f32 * 13.0 / max_damage as f32 + 0.5).clamp(0, 13);
    let health = ((max_damage as f32 - damage as f32) / max_damage as f32).max(0.0);
    Some((width, hsv_to_argb(health / 3.0, 1.0, 1.0, 0)))
}

/// What `itemDecorations` reads from `stack`: its count and `isBarVisible` bar (a bundle's weight,
/// else its damage); the cooldown is the player's, so none here.
pub fn of_stack(stack: &ItemStack, game: &Game, count_text: Option<&str>) -> ItemDecorations {
    let bar = if stack.id.ends_with("bundle") {
        bundle::bar(stack, game)
    } else {
        // isDamageableItem: max_damage and damage, not unbreakable; getDamageValue clamps.
        let max = if stack.has("unbreakable") || !stack.has("damage") { 0 } else { stack.int("max_damage", 0) };
        damage_bar(stack.int("damage", 0).min(max), max)
    };
    ItemDecorations { count: stack.count, count_text: count_text.map(str::to_owned), bar, cooldown: 0.0 }
}

/// `ItemCooldowns.getCooldownPercent` for a cooldown `elapsed` whole ticks into `duration`; it is
/// gone once `elapsed` reaches `duration`.
pub fn cooldown_percent(duration: i32, elapsed: i32, partial: f32) -> f32 {
    if elapsed >= duration {
        return 0.0;
    }
    clamp((duration as f32 - (elapsed as f32 + partial)) / duration as f32, 0.0, 1.0)
}

/// `itemDecorations` over a stack drawn at (x, y).
pub fn item_decorations(gui: &mut Gui, font: &dyn Metrics, x: i32, y: i32, d: &ItemDecorations) {
    gui.push(Vec::new());
    if let Some((width, color)) = d.bar {
        let (left, top) = (x + 2, y + 13);
        gui.fill(left, top, left + 13, top + 2, BLACK);
        gui.fill(left, top, left + width, top + 1, color | BLACK);
    }
    if d.cooldown > 0.0 {
        let top = y + floor(16.0 * (1.0 - d.cooldown));
        let bottom = top + ceil(16.0 * d.cooldown);
        gui.fill(x, top, x + 16, bottom, 0x7FFF_FFFF);
    }
    if d.count != 1 || d.count_text.is_some() {
        let amount = d.count_text.clone().unwrap_or_else(|| d.count.to_string());
        let width = font.width(&amount.as_str().into());
        gui.text(amount, x + 19 - 2 - width, y + 6 + 3, WHITE, true);
    }
    gui.pop();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durability_bar() {
        assert_eq!(damage_bar(0, 100), None);
        assert_eq!(damage_bar(50, 100).map(|b| b.0), Some(7));
        assert_eq!(damage_bar(100, 100), Some((0, 0xFF0000)));
        assert_eq!(damage_bar(1, 1561), Some((13, 0x00FF00)));
    }

    #[test]
    fn a_stack_shows_its_bundle_weight_or_its_damage() {
        let game = Game::new(serde_json::json!({"item": {"minecraft:stone": {"components": {"minecraft:max_stack_size": 64}}}}));
        let stack = |id: &str, components: serde_json::Value| ItemStack::from_json(&serde_json::json!({"id": id, "count": 1, "components": components})).unwrap();
        let half = stack("minecraft:bundle", serde_json::json!({"minecraft:bundle_contents": [{"id": "minecraft:stone", "count": 32}]}));
        assert_eq!(of_stack(&half, &game, None).bar, Some((7, 0x7087FF)));
        let full = stack("minecraft:red_bundle", serde_json::json!({"minecraft:bundle_contents": [{"id": "minecraft:stone", "count": 64}]}));
        assert_eq!(of_stack(&full, &game, None).bar, Some((13, 0xFF5454)));
        assert_eq!(of_stack(&stack("minecraft:bundle", serde_json::json!({"minecraft:bundle_contents": []})), &game, None).bar, None);
        let worn = serde_json::json!({"minecraft:max_damage": 100, "minecraft:damage": 50});
        assert_eq!(of_stack(&stack("minecraft:iron_sword", worn.clone()), &game, None).bar.map(|b| b.0), Some(7));
        let mut unbreakable = worn;
        unbreakable["minecraft:unbreakable"] = serde_json::json!({});
        assert_eq!(of_stack(&stack("minecraft:iron_sword", unbreakable), &game, Some("9")).bar, None);
    }

    #[test]
    fn cooldown_runs_out() {
        assert_eq!(cooldown_percent(20, 0, 0.0), 1.0);
        assert_eq!(cooldown_percent(20, 10, 0.5), 0.475);
        assert_eq!(cooldown_percent(20, 20, 0.0), 0.0);
    }
}
