//! The `screens_lists` harness: the key binds, language and pack screens; the schema is its
//! `cases.py`'s.

use acacia_ui::java::gui::{Gui, GuiDraw};
use acacia_ui::java::screen::keybinds::{self, KeyBinds};
use acacia_ui::java::screen::keys::{Key, Os, defaults};
use acacia_ui::java::screen::language::{self, Language, Languages};
use acacia_ui::java::screen::packs::{self, Compat, Pack, Packs};
use acacia_ui::java::screen::{Env, extract};
use acacia_ui::java::text::json_root_style;
use acacia_ui::java::widget::Ctx;
use serde_json::{Map, Value};

use super::{flag, int, ints, lang, text};
use crate::Assets;

/// `Util.getMillis()` in the probe's clock; the caret blink only reads differences.
const MILLIS: i64 = 1_000_000_000;

fn str_of(v: &Value) -> &str {
    v.as_str().unwrap_or_default()
}

fn key(v: &Value) -> Key {
    let value = v[1].as_i64().unwrap_or(0) as i32;
    if str_of(&v[0]) == "mouse" { Key::mouse(value) } else { Key::keyboard(value) }
}

fn pack(v: &Value, selected: bool, lang: &std::collections::HashMap<String, String>) -> Pack {
    let compat = match str_of(&v["compat"]) {
        "TOO_OLD" => Compat::TooOld,
        "TOO_NEW" => Compat::TooNew,
        "UNKNOWN" => Compat::Unknown,
        _ => Compat::Compatible,
    };
    Pack {
        id: str_of(&v["id"]).to_owned(),
        title: text(&v["title"], lang),
        description: text(&v["description"], lang),
        description_style: json_root_style(&v["description"]),
        icon: str_of(&v["icon"]).to_owned(),
        compat,
        selected,
        required: flag(v, "required", false),
        fixed: flag(v, "fixed", false),
        can_move_up: flag(v, "up", false),
        can_move_down: flag(v, "down", false),
    }
}

pub fn replay(c: &Value, assets: &Assets, _: &mut Map<String, Value>) -> Vec<GuiDraw> {
    let lang = lang(assets);
    let tr = |k: &str| lang.get(k).cloned();
    let env = Env { width: int(c, "width", 427), height: int(c, "height", 240), in_world: true, blur: int(c, "blur", 5) >= 1 };
    let mut gui = Gui::new(env.width, env.height);
    let mouse = ints(&c["mouse"]);
    let mut ctx = Ctx::new(&mut gui, assets.font.as_ref(), &tr, (mouse[0], mouse[1]));
    ctx.millis = MILLIS;
    let focused_time = MILLIS - i64::from(int(c, "focusAgo", 0));
    let mut screen = match str_of(&c["kind"]) {
        "keybinds" => {
            let mut mappings = defaults();
            if let Some(keys) = c["keys"].as_object() {
                for m in &mut mappings {
                    if let Some(k) = keys.get(m.name) {
                        m.key = key(k);
                    }
                }
            }
            let k = KeyBinds { mappings: &mappings, selected: c["selected"].as_str(), os: Os::Linux, drawn_before: flag(c, "warm", false) };
            let mut s = keybinds::build(&ctx, env, &k);
            s.lists[0].set_scroll(f64::from(int(c, "scroll", 0)));
            s
        }
        "language" => {
            let languages: Vec<Language> = c["languages"].as_array().into_iter().flatten()
                .map(|l| Language { code: str_of(&l[0]).into(), name: str_of(&l[1]).into(), region: str_of(&l[2]).into() })
                .collect();
            let list_focused = flag(c, "listFocus", false);
            let l = Languages {
                languages: &languages,
                selected: str_of(&c["selected"]),
                search: str_of(&c["search"]),
                search_focused: !list_focused,
                focused_time,
                list_focused,
                scroll: c["scroll"].as_f64(),
            };
            language::build(&ctx, env, &l)
        }
        "packs" => {
            let list = |k: &str, selected: bool| c[k].as_array().into_iter().flatten().map(|p| pack(p, selected, lang)).collect::<Vec<_>>();
            let (available, selected) = (list("available", false), list("selected", true));
            let focus = c["focus"].as_array().map(|f| (f[0].as_u64().unwrap_or(0) as usize, f[1].as_u64().unwrap_or(0) as usize));
            let scroll = ints(&c["scroll"]);
            let p = Packs {
                title: text(&c["title"], lang),
                available: &available,
                selected: &selected,
                search: str_of(&c["search"]),
                search_focused: true,
                focused_time,
                scroll: [f64::from(scroll[0]), f64::from(scroll[1])],
                focus,
            };
            packs::build(&ctx, env, &p)
        }
        _ => return Vec::new(),
    };
    extract(&mut screen, &mut ctx);
    drop(ctx);
    gui.draws
}
