//! The `screens_more` harness: one loading, message, credits or win screen per case; the schema is
//! its `cases.py`'s.

use std::sync::OnceLock;

use acacia_ui::java::gui::{Gui, GuiDraw};
use acacia_ui::java::screen::credits::credits_and_attribution;
use acacia_ui::java::screen::loading::{ChunkView, GenericMessageScreen, LevelLoadingScreen, Load, Reason};
use acacia_ui::java::screen::win::{WinScreen, lines, scroll_speed};
use acacia_ui::java::screen::{Env, Screen, extract};
use acacia_ui::java::widget::Ctx;
use serde_json::{Map, Value};

use super::{flag, float, int, ints, lang, text};
use crate::Assets;

/// `texts/<name>` from the jar's assets.
fn texts(assets: &Assets, name: &str) -> String {
    std::fs::read_to_string(assets.root.join("texts").join(name)).unwrap_or_default()
}

fn credits(assets: &Assets) -> &'static Value {
    static CREDITS: OnceLock<Value> = OnceLock::new();
    CREDITS.get_or_init(|| serde_json::from_str(&texts(assets, "credits.json")).unwrap_or(Value::Null))
}

fn loading(c: &Value) -> LevelLoadingScreen {
    let reason = match c["reason"].as_str() {
        Some("NETHER_PORTAL") => Reason::NetherPortal,
        Some("END_PORTAL") => Reason::EndPortal,
        _ => Reason::Other,
    };
    let view = c.get("view").map(|v| ChunkView {
        radius: int(v, "radius", 0),
        cells: ints(&v["cells"]).into_iter().map(|s| usize::try_from(s).ok()).collect(),
    });
    let load = Load { has_progress: flag(c, "hasProgress", false), progress: float(c, "progress", 0.0), view, visualize: flag(c, "visualize", false) };
    let mut screen = LevelLoadingScreen::new(env(c), reason, load);
    for _ in 0..int(c, "ticks", 0) {
        screen.tick();
    }
    screen
}

fn env(c: &Value) -> Env {
    Env { width: int(c, "width", 427), height: int(c, "height", 240), in_world: flag(c, "inWorld", true), blur: int(c, "blur", 5) >= 1 }
}

pub fn replay(c: &Value, assets: &Assets, _: &mut Map<String, Value>) -> Vec<GuiDraw> {
    let lang = lang(assets);
    let tr = |k: &str| lang.get(k).cloned();
    let env = env(c);
    let mut gui = Gui::new(env.width, env.height);
    let mouse = ints(&c["mouse"]);
    let mut ctx = Ctx::new(&mut gui, assets.font.as_ref(), &tr, (mouse[0], mouse[1]));
    let mut screen: Box<dyn Screen> = match c["kind"].as_str().unwrap_or_default() {
        "loading" => Box::new(loading(c)),
        "message" => Box::new(GenericMessageScreen::new(&ctx, env, text(&c["title"], lang))),
        "credits" => Box::new(credits_and_attribution(&ctx, env)),
        "win" => {
            let poem = flag(c, "poem", false);
            let (end, post) = (texts(assets, "end.txt"), texts(assets, "postcredits.txt"));
            let player = c["player"].as_str().unwrap_or("Steve");
            let mut s = WinScreen::new(env, poem, lines(&ctx, poem.then_some((end.as_str(), post.as_str(), player)), credits(assets)));
            s.scroll = float(c, "scroll", 0.0);
            s.speed = scroll_speed(poem, flag(c, "speedup", false), int(c, "modifiers", 0) as usize, flag(c, "reverse", false));
            s.partial = float(c, "partial", 0.0);
            Box::new(s)
        }
        _ => return Vec::new(),
    };
    extract(screen.as_mut(), &mut ctx);
    drop(ctx);
    gui.draws
}
