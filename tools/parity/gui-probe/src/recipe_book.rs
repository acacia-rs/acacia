//! The `recipe_book` harness's kinds; the case schema is `tools/parity/harnesses/recipe_book/cases.py`'s,
//! with `ScreenKit`'s resolved `items` and `RecipeBookState`'s `game` (tabs, collections, ghost slots).

use serde_json::{Map, Value};

use acacia_ui::java::container::entity::Figure;
use acacia_ui::java::container::recipe_book::screens::{RecipeScreen, Window};
use acacia_ui::java::container::recipe_book::{Book, Cell, Collection, Entry, Ghost, Kind, Search, Tab};
use acacia_ui::java::container::{Env, frame};
use acacia_ui::java::gui::{Gui, GuiDraw};
use acacia_ui::java::text::Text;
use acacia_ui::java::tooltip::Deferred;
use acacia_ui::java::tooltip::stack::ItemStack;

use crate::Assets;
use crate::kit;

fn ints(v: &Value) -> Vec<i32> {
    v.as_array().into_iter().flatten().filter_map(Value::as_i64).map(|i| i as i32).collect()
}

fn floats(v: &Value) -> Vec<f32> {
    v.as_array().into_iter().flatten().filter_map(Value::as_f64).map(|f| f as f32).collect()
}

fn entry(e: &Value) -> Entry {
    let cells = e["cells"].as_array().into_iter().flatten();
    let cells = cells.map(|c| Cell { x: c[0].as_i64().unwrap_or(0) as i32, y: c[1].as_i64().unwrap_or(0) as i32, items: ints(&c[2]) });
    Entry { selected: kit::bool(e, "selected"), craftable: kit::bool(e, "craftable"), display: ints(&e["display"]), cells: cells.collect() }
}

fn book(c: &Value, kind: Kind) -> Book {
    let g = &c["game"];
    let open = kit::bool(c, "open");
    let anims = floats(&c["tabAnims"]);
    let tabs = g["tabs"].as_array().into_iter().flatten().enumerate();
    let tabs = tabs.map(|(i, t)| Tab { search: kit::bool(t, "search"), icons: ints(&t["icons"]), collections: ints(&t["collections"]).into_iter().map(|c| c as usize).collect(), anim: anims.get(i).copied().unwrap_or(0.0) });
    let collections = g["collections"].as_array().into_iter().flatten();
    let collections = collections.map(|c| Collection { entries: c["entries"].as_array().into_iter().flatten().map(entry).collect() });
    let ghost = g["ghost"].as_array().into_iter().flatten();
    let ghost = ghost.map(|g| Ghost { slot: g["slot"].as_u64().unwrap_or(0) as usize, result: kit::bool(g, "result"), items: ints(&g["items"]) });
    let s = &c["search"];
    let search = Search {
        value: s["value"].as_str().unwrap_or_default().to_owned(),
        cursor: s["cursor"].as_u64().unwrap_or(0) as usize,
        highlight: s["highlight"].as_u64().unwrap_or(0) as usize,
        display: s["display"].as_u64().unwrap_or(0) as usize,
        focused: kit::bool(s, "focused"),
        blink_ms: s["blink"].as_u64().unwrap_or(0),
    };
    let items = c["items"].as_object().into_iter().flatten();
    let items = items.filter_map(|(k, v)| Some((k.parse().ok()?, ItemStack::from_json(v)?)));
    let width = kit::int(c, "width", 0);
    Book {
        kind,
        visible: open,
        narrow: width < 379,
        width,
        height: kit::int(c, "height", 0),
        filtering: kit::bool(c, "filtering"),
        time: if open { kit::float(c, "time", 0.0) } else { 0.0 },
        tabs: tabs.collect(),
        selected_tab: g["selectedTab"].as_u64().unwrap_or(0) as usize,
        collections: collections.collect(),
        page: c["page"].as_u64().unwrap_or(0) as usize,
        button_anims: floats(&c["buttonAnims"]),
        search,
        overlay: g["overlay"].as_u64().map(|b| b as usize),
        ghost: ghost.collect(),
        items: items.collect(),
        hovered: None,
    }
}

pub fn replay(c: &Value, assets: &Assets, _state: &mut Map<String, Value>) -> Vec<GuiDraw> {
    let lang = kit::lang(assets);
    let game = kit::game(c);
    let env = Env { font: assets.font.as_ref(), lang: &lang, advanced: kit::bool(c, "advanced"), game: &game, op: kit::bool(c, "op") };
    let kind = c["kind"].as_str().unwrap_or_default();
    let (bare, screen) = kind.strip_prefix("book_").map_or((false, kind), |s| (true, s));
    let (book_kind, title) = match screen {
        "furnace" => (Kind::Furnace, "container.furnace"),
        "blast_furnace" => (Kind::BlastFurnace, "container.blast_furnace"),
        "smoker" => (Kind::Smoker, "container.smoker"),
        _ => (Kind::Crafting, "container.crafting"),
    };
    let mut book = book(c, book_kind);
    let mut gui = Gui::new(book.width, book.height);
    let (mouse, partial) = (kit::mouse(c), kit::float(c, "partial", 0.0));
    if bare {
        let mut deferred = Deferred::default();
        book.render(&mut gui, &env, mouse, partial, &mut deferred);
        book.tooltip(&env, mouse, None, &mut deferred);
        deferred.extract(&mut gui, env.font);
        return gui.draws;
    }
    let window = match screen {
        "crafting" => Window::Crafting,
        "inventory" => {
            let f = &c["figure"];
            let figure = Figure { entity: "minecraft:player".into(), bb_height: kit::float(f, "bbHeight", 1.8), scale: Some(kit::float(f, "scale", 1.0)), fall_flying: kit::bool(f, "fallFlying") };
            let last = floats(&c["last"]);
            Window::Inventory { figure, last_mouse: [last.first().copied().unwrap_or(0.0), last.get(1).copied().unwrap_or(0.0)], effects: kit::effects(c) }
        }
        _ => {
            let mut data = [0; 4];
            for (d, v) in data.iter_mut().zip(ints(&c["data"])) {
                *d = v;
            }
            Window::Furnace(data)
        }
    };
    let inventory = Text::translate("container.inventory", &[], &lang);
    let mut s = RecipeScreen::new(book, window, Text::translate(title, &[], &lang), inventory, &env);
    kit::fill(c, &mut s.base);
    frame(&mut s, &mut gui, &env, mouse, partial);
    gui.draws
}
