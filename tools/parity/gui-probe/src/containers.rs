//! The `containers` harness's kinds; the case schema is `tools/parity/harnesses/containers/cases.py`'s,
//! with `ScreenKit`'s resolved `items`.

use serde_json::{Map, Value};

use acacia_ui::java::container::brewing::Brewing;
use acacia_ui::java::container::crafter::Crafter;
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
    let mut screen: Box<dyn Screen> = match c["kind"].as_str().unwrap_or_default() {
        "chest" => Box::new(simple(Kind::Chest { rows: kit::int(c, "rows", 3) }, "container.chest")),
        "shulker" => Box::new(simple(Kind::Shulker, "container.shulkerBox")),
        "dispenser" => Box::new(simple(Kind::Dispenser, "container.dispenser")),
        "hopper" => Box::new(simple(Kind::Hopper, "container.hopper")),
        "brewing" => Box::new(Brewing::new(width, height, kit::title(c, "container.brewing", &lang), inventory, data(c), env.font)),
        "crafter" => Box::new(Crafter::new(width, height, kit::title(c, "container.crafter", &lang), inventory, data(c), env.font)),
        // A kind not ported yet draws nothing, so it shows as a divergence.
        _ => return Vec::new(),
    };
    kit::fill(c, screen.base_mut());
    let mut gui = Gui::new(width, height);
    frame(screen.as_mut(), &mut gui, &env, kit::mouse(c), kit::float(c, "partial", 0.0));
    gui.draws
}

/// The case's menu data slots, zero beyond those given.
fn data<const N: usize>(c: &Value) -> [i32; N] {
    let mut out = [0; N];
    for (o, v) in out.iter_mut().zip(c["data"].as_array().into_iter().flatten()) {
        *o = v.as_i64().unwrap_or(0) as i32;
    }
    out
}
