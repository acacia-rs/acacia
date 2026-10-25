//! The `trading` harness's kinds; the case schema is `tools/parity/harnesses/trading/cases.py`'s,
//! with `ScreenKit`'s resolved `items` and the menu state `TradingOracle` adds (`menu`).

use serde_json::{Map, Value};

use acacia_ui::java::container::loom::{Loom, Pattern};
use acacia_ui::java::container::merchant::{Merchant, Offer};
use acacia_ui::java::container::stonecutter::Stonecutter;
use acacia_ui::java::container::{Base, CARRIED, Env, Screen, frame};
use acacia_ui::java::gui::{Gui, GuiDraw};
use acacia_ui::java::text::Text;
use acacia_ui::java::tooltip::stack::ItemStack;

use crate::Assets;
use crate::kit;

/// `TradingScreens.OFFERS`, `RECIPES`: where list entries' keys start.
const OFFERS: i32 = 1000;
const RECIPES: i32 = 2000;

pub fn replay(c: &Value, assets: &Assets, _state: &mut Map<String, Value>) -> Vec<GuiDraw> {
    let lang = kit::lang(assets);
    let env = Env { font: assets.font.as_ref(), lang: &lang, advanced: kit::bool(c, "advanced") };
    let (width, height) = (kit::int(c, "width", 0), kit::int(c, "height", 0));
    let inventory = Text::translate("container.inventory", &[], &lang);
    let mut screen: Box<dyn Screen> = match c["kind"].as_str().unwrap_or_default() {
        "merchant" => {
            let mut s = Merchant::new(width, height, kit::title(c, "entity.minecraft.villager", &lang), inventory);
            fill(c, &mut s.base);
            s.offers = offers(c);
            (s.level, s.xp, s.future_xp) = (kit::int(c, "level", 0), kit::int(c, "xp", 0), kit::int(&c["menu"], "futureXp", 0));
            (s.show_progress_bar, s.can_restock) = (kit::bool(c, "bar"), kit::bool(c, "restock"));
            (s.shop_item, s.scroll_off) = (kit::int(c, "shop", 0) as usize, kit::int(c, "scroll", 0) as usize);
            Box::new(s)
        }
        "stonecutter" => {
            let mut s = Stonecutter::new(width, height, kit::title(c, "container.stonecutter", &lang), inventory);
            fill(c, &mut s.base);
            let icons: Vec<ItemStack> = (0..kit::int(&c["menu"], "recipes", 0)).filter_map(|i| kit::item(c, RECIPES + i)).collect();
            s.recipes = icons.iter().enumerate().map(|(i, icon)| (icon.clone(), RECIPES + first_equal(&icons, i) as i32)).collect();
            s.selected = kit::int(&c["menu"], "selected", -1);
            s.container_changed();
            for dy in scrolls(c) {
                s.scroll(dy);
            }
            Box::new(s)
        }
        "loom" => {
            let mut s = Loom::new(width, height, kit::title(c, "container.loom", &lang), inventory);
            fill(c, &mut s.base);
            let patterns = c["menu"]["patterns"].as_array().into_iter().flatten();
            s.patterns = patterns.map(|p| Pattern { translation_key: str(&p[0]), sprite: str(&p[1]) }).collect();
            s.selected = kit::int(&c["menu"], "selected", -1);
            s.container_changed();
            for dy in scrolls(c) {
                s.scroll(dy);
            }
            Box::new(s)
        }
        // A kind not ported yet draws nothing, so it shows as a divergence.
        _ => return Vec::new(),
    };
    let mut gui = Gui::new(width, height);
    frame(screen.as_mut(), &mut gui, &env, kit::mouse(c), kit::float(c, "partial", 0.0));
    gui.draws
}

fn str(v: &Value) -> String {
    v.as_str().unwrap_or_default().to_owned()
}

/// Every slot's stack as the menu held it (keyed by index), and the carried stack.
fn fill(c: &Value, base: &mut Base) {
    for (i, slot) in base.slots.iter_mut().enumerate() {
        slot.item = kit::item(c, i as i32);
    }
    base.carried = kit::item(c, CARRIED);
}

/// The index of the first stack equal to `stacks[i]`: the oracle keys copies by content.
fn first_equal(stacks: &[ItemStack], i: usize) -> usize {
    stacks.iter().position(|s| *s == stacks[i]).unwrap_or(i)
}

fn scrolls(c: &Value) -> Vec<f64> {
    c["scrolls"].as_array().into_iter().flatten().filter_map(Value::as_f64).collect()
}

/// The case's `MerchantOffer.CODEC` offers over their resolved stacks.
fn offers(c: &Value) -> Vec<Offer> {
    let mut out: Vec<Offer> = Vec::new();
    for (i, o) in c["offers"].as_array().into_iter().flatten().enumerate() {
        let key = OFFERS + 4 * i as i32;
        let mut offer = Offer {
            base_cost_a: kit::item(c, key).unwrap_or_default(),
            cost_b: kit::item(c, key + 2),
            result: kit::item(c, key + 3).unwrap_or_default(),
            uses: kit::int(o, "uses", 0),
            max_uses: kit::int(o, "maxUses", 4),
            special_price: kit::int(o, "specialPrice", 0),
            demand: kit::int(o, "demand", 0),
            price_multiplier: kit::float(o, "priceMultiplier", 0.0),
            keys: [key, key + 1, key + 2, key + 3],
        };
        let cost = offer.cost_a();
        if let Some(j) = out.iter().position(|p| p.cost_a() == cost) {
            offer.keys[1] = OFFERS + 4 * j as i32 + 1;
        }
        out.push(offer);
    }
    out
}
