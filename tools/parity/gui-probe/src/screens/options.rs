//! The `screens_options` harness: one options screen per case; the schema is its `cases.py`'s.

use acacia_ui::java::gui::{Gui, GuiDraw};
use acacia_ui::java::screen::options::pages::{Client, build};
use acacia_ui::java::screen::options::value::V;
use acacia_ui::java::screen::{Env, extract};
use acacia_ui::java::widget::Ctx;
use serde_json::{Map, Value};

use super::{flag, int, ints, lang};
use crate::Assets;

fn option(v: &Value) -> Option<V> {
    Some(match v {
        Value::Bool(b) => V::Bool(*b),
        Value::String(s) => V::Name(s.clone()),
        Value::Number(n) if n.is_i64() => V::Int(n.as_i64()? as i32),
        Value::Number(n) => V::Num(n.as_f64()?),
        _ => return None,
    })
}

pub fn replay(c: &Value, assets: &Assets, _: &mut Map<String, Value>) -> Vec<GuiDraw> {
    let lang = lang(assets);
    let tr = |k: &str| lang.get(k).cloned();
    let blur = int(&c["values"], "menuBackgroundBlurriness", 5) >= 1;
    let env = Env { width: int(c, "width", 427), height: int(c, "height", 240), in_world: true, blur };
    let mut gui = Gui::new(env.width, env.height);
    let mouse = ints(&c["mouse"]);
    let mut ctx = Ctx::new(&mut gui, assets.font.as_ref(), &tr, (mouse[0], mouse[1]));
    let values = |id: &str| option(&c["values"][id]);
    let client = Client {
        telemetry: flag(c, "telemetry", false),
        high_contrast_pack: flag(c, "highContrastPack", false),
        minecart: flag(c, "minecart", false),
        friends: flag(c, "friends", false),
        friend_requests: flag(c, "friendRequests", false),
        ticked: flag(c, "tick", false),
    };
    let Some(mut screen) = build(&ctx, env, c["kind"].as_str().unwrap_or_default(), &values, client) else {
        return Vec::new();
    };
    if let Some(list) = &mut screen.list {
        list.list.set_scroll(f64::from(int(c, "scroll", 0)));
    }
    extract(&mut screen, &mut ctx);
    drop(ctx);
    gui.draws
}
