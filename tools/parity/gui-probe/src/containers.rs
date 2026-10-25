//! The `containers` harness's kinds; the case schema is `tools/parity/harnesses/containers/cases.py`'s,
//! with `ScreenKit`'s resolved `items`.

use serde_json::{Map, Value};

use acacia_ui::java::container::brewing::Brewing;
use acacia_ui::java::container::crafter::Crafter;
use acacia_ui::java::container::effects;
use acacia_ui::java::container::entity::Figure;
use acacia_ui::java::container::lectern::Lectern;
use acacia_ui::java::container::mount::{self, Mount};
use acacia_ui::java::container::simple::{Kind, Simple};
use acacia_ui::java::container::{Env, Screen, frame};
use acacia_ui::java::gui::{Gui, GuiDraw};
use acacia_ui::java::text::Text;

use crate::Assets;
use crate::kit;

pub fn replay(c: &Value, assets: &Assets, _state: &mut Map<String, Value>) -> Vec<GuiDraw> {
    let lang = kit::lang(assets);
    let env = Env { font: assets.font.as_ref(), lang: &lang, advanced: kit::bool(c, "advanced") };
    let (width, height) = (kit::int(c, "width", 0), kit::int(c, "height", 0));
    let inventory = Text::translate("container.inventory", &[], &lang);
    let simple = |kind, fallback| Simple::new(kind, width, height, kit::title(c, fallback, &lang), inventory.clone(), env.font);
    if c["kind"] == "lectern" {
        let pages = c["game"]["pages"].as_array().into_iter().flatten().map(|p| Text::from_json(p, &lang)).collect();
        let lectern = Lectern { width, height, pages, page: kit::int(&c["game"], "page", 0), may_build: c["build"].as_bool().unwrap_or(true) };
        let mut gui = Gui::new(width, height);
        lectern.frame(&mut gui, env.font, &lang, kit::mouse(c));
        return gui.draws;
    }
    if c["kind"] == "effects" {
        let mut screen = simple(Kind::Chest { rows: kit::int(c, "rows", 3) }, "container.chest");
        let mut gui = Gui::new(width, height);
        effects::extract(&mut screen.base, &mut gui, env.font, &lang, &kit::effects(c), kit::mouse(c), 20.0);
        screen.base.tooltip.extract(&mut gui, env.font);
        return gui.draws;
    }
    let mut screen: Box<dyn Screen> = match c["kind"].as_str().unwrap_or_default() {
        "chest" => Box::new(simple(Kind::Chest { rows: kit::int(c, "rows", 3) }, "container.chest")),
        "shulker" => Box::new(simple(Kind::Shulker, "container.shulkerBox")),
        "dispenser" => Box::new(simple(Kind::Dispenser, "container.dispenser")),
        "hopper" => Box::new(simple(Kind::Hopper, "container.hopper")),
        "brewing" => Box::new(Brewing::new(width, height, kit::title(c, "container.brewing", &lang), inventory, data(c), env.font)),
        "crafter" => Box::new(Crafter::new(width, height, kit::title(c, "container.crafter", &lang), inventory, data(c), env.font)),
        "mount" => Box::new(mount(c, width, height, inventory, &lang)),
        // A kind not ported yet draws nothing, so it shows as a divergence.
        _ => return Vec::new(),
    };
    kit::fill(c, screen.base_mut());
    let mut gui = Gui::new(width, height);
    frame(screen.as_mut(), &mut gui, &env, kit::mouse(c), kit::float(c, "partial", 0.0));
    gui.draws
}

/// `ContainerScreens.mount`: what the oracle found of the mount in `game`.
fn mount(c: &Value, width: i32, height: i32, inventory: Text, lang: &dyn Fn(&str) -> Option<String>) -> Mount {
    let g = &c["game"];
    let kind = match g["mount"].as_str() {
        Some("llama") => mount::Kind::Llama,
        Some("horse") => mount::Kind::Horse,
        _ => mount::Kind::Nautilus,
    };
    let f = &g["figure"];
    let figure = Figure {
        entity: f["entity"].as_str().unwrap_or_default().to_owned(),
        bb_height: kit::float(f, "bbHeight", 0.0),
        scale: f["scale"].as_f64().map(|s| s as f32),
        fall_flying: kit::bool(f, "fallFlying"),
    };
    let title = Text::from_json(&g["title"], lang);
    let saddle = kit::bool(g, "saddle");
    let mut screen = Mount::new(kind, width, height, title, inventory, kit::int(c, "columns", 0), saddle, kit::bool(g, "armor"), figure);
    let last = &c["lastMouse"];
    screen.last_mouse = [last[0].as_f64().unwrap_or(0.0) as f32, last[1].as_f64().unwrap_or(0.0) as f32];
    screen
}

/// The case's menu data slots, zero beyond those given.
fn data<const N: usize>(c: &Value) -> [i32; N] {
    let mut out = [0; N];
    for (o, v) in out.iter_mut().zip(c["data"].as_array().into_iter().flatten()) {
        *o = v.as_i64().unwrap_or(0) as i32;
    }
    out
}
