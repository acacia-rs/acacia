//! The `stations` harness's kinds; the case schema is `tools/parity/harnesses/stations/cases.py`'s,
//! with `ScreenKit`'s resolved `items` (every slot's final stack, by index) and `game`: the menu's
//! final data slots (`data`), the enchanting clues' enchantments (`clues`) and whether the
//! cartography table's map is extendable (`extendable`).

use serde_json::{Map, Value};

use acacia_ui::java::container::anvil::Anvil;
use acacia_ui::java::container::beacon::Beacon;
use acacia_ui::java::container::cartography::{Cartography, MapData};
use acacia_ui::java::container::enchanting::{Clue, Enchanting};
use acacia_ui::java::container::grindstone::Grindstone;
use acacia_ui::java::container::item_combiner::Station;
use acacia_ui::java::container::smithing::Smithing;
use acacia_ui::java::container::{Base, CARRIED, Env, Screen, frame};
use acacia_ui::java::gui::{Gui, GuiDraw};
use acacia_ui::java::text::{Lang, Text};

use crate::Assets;
use crate::kit;

pub fn replay(c: &Value, assets: &Assets, _state: &mut Map<String, Value>) -> Vec<GuiDraw> {
    let lang = kit::lang(assets);
    let env = Env { font: assets.font.as_ref(), lang: &lang, advanced: kit::bool(c, "advanced") };
    let (width, height) = (kit::int(c, "width", 0), kit::int(c, "height", 0));
    let inventory = Text::translate("container.inventory", &[], &lang);
    let title = |fallback| kit::title(c, fallback, &lang);
    let station = Station {
        xp: kit::int(c, "xp", 0),
        creative: kit::bool(c, "creative"),
        data: c["game"]["data"].as_array().into_iter().flatten().map(|v| v.as_i64().unwrap_or(0) as i32).collect(),
    };
    let mut screen: Box<dyn Screen> = match c["kind"].as_str().unwrap_or_default() {
        "anvil" => {
            let base = filled(c, Base::new(width, height, [176, 166], title("container.repair"), inventory, Anvil::slots()));
            let typed = c["name"].as_str();
            Box::new(Anvil::new(base, station, typed, !kit::bool(c, "blink"), env.font, &lang))
        }
        "grindstone" => Box::new(Grindstone::new(width, height, title("container.grindstone_title"), inventory)),
        "smithing" => Box::new(Smithing::new(width, height, title("container.upgrade"), inventory, station, kit::int(c, "ticks", 0))),
        "cartography" => Box::new(Cartography::new(width, height, title("container.cartography_table"), inventory, map(c), kit::bool(&c["game"], "extendable"))),
        "enchanting" => {
            let pair = |key: &str| [c[key][0].as_f64().unwrap_or(0.0) as f32, c[key][1].as_f64().unwrap_or(0.0) as f32];
            Box::new(Enchanting::new(width, height, title("container.enchant"), inventory, station, clues(c, &lang), pair("open"), pair("flip")))
        }
        "beacon" => {
            let effect = |key: &str| c[key].as_str().map(|id| id.trim_start_matches("minecraft:").to_owned());
            let (primary, secondary) = (effect("primary"), effect("secondary"));
            let payment = kit::item(c, 0).is_some_and(|s| !s.is_empty());
            Box::new(Beacon::new(width, height, title("container.beacon"), inventory, &station, primary.as_deref(), secondary.as_deref(), payment))
        }
        // A kind not ported yet draws nothing, so it shows as a divergence.
        _ => return Vec::new(),
    };
    fill(c, screen.base_mut());
    let mut gui = Gui::new(width, height);
    frame(screen.as_mut(), &mut gui, &env, kit::mouse(c), kit::float(c, "partial", 0.0));
    gui.draws
}

/// Every slot's final stack and the carried one, as the oracle keyed them.
fn fill(c: &Value, base: &mut Base) {
    for (i, slot) in base.slots.iter_mut().enumerate() {
        slot.item = kit::item(c, i as i32);
    }
    base.carried = kit::item(c, CARRIED);
}

fn filled(c: &Value, mut base: Base) -> Base {
    fill(c, &mut base);
    base
}

/// The data of the first slot's map: its `map_id` among the case's `maps` ([id, scale, locked]).
fn map(c: &Value) -> Option<MapData> {
    let id = kit::item(c, 0)?.get("map_id")?.as_i64()?;
    let entry = c["maps"].as_array()?.iter().find(|m| m[0].as_i64() == Some(id))?;
    Some(MapData { scale: entry[1].as_i64().unwrap_or(0) as i32, locked: entry[2].as_bool().unwrap_or(false), decorations: 0 })
}

fn clues(c: &Value, lang: Lang) -> [Option<Clue>; 3] {
    std::array::from_fn(|i| {
        let clue = &c["game"]["clues"][i];
        clue.is_object().then(|| Clue {
            description: Text::from_json(&clue["description"], lang),
            curse: clue["curse"].as_bool().unwrap_or(false),
            max_level: clue["maxLevel"].as_i64().unwrap_or(1) as i32,
        })
    })
}
