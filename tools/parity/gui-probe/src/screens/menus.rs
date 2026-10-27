//! The `screens_menus` harness: one menu screen per case; the schema is its `cases.py`'s.

use acacia_ui::java::gui::{Gui, GuiDraw};
use acacia_ui::java::screen::confirm::{Confirm, ConfirmScreen};
use acacia_ui::java::screen::death::DeathScreen;
use acacia_ui::java::screen::message::MessageScreen;
use acacia_ui::java::screen::pause::{Pause, PauseScreen};
use acacia_ui::java::screen::{Env, Screen, extract};
use acacia_ui::java::text::json_root_style;
use acacia_ui::java::widget::Ctx;
use serde_json::{Map, Value};

use super::{flag, int, ints, lang, text};
use crate::Assets;

pub fn replay(c: &Value, assets: &Assets, _: &mut Map<String, Value>) -> Vec<GuiDraw> {
    let lang = lang(assets);
    let tr = |k: &str| lang.get(k).cloned();
    let env = Env { width: int(c, "width", 427), height: int(c, "height", 240), in_world: true, blur: int(c, "blur", 5) >= 1 };
    let mut gui = Gui::new(env.width, env.height);
    let mouse = ints(&c["mouse"]);
    let mut ctx = Ctx::new(&mut gui, assets.font.as_ref(), &tr, (mouse[0], mouse[1]));
    let t = |k: &str| text(&c[k], lang);
    let ticks = int(c, "ticks", 0) as u32;
    let mut screen: Box<dyn Screen> = match c["kind"].as_str().unwrap_or_default() {
        "pause" => Box::new(PauseScreen::new(&ctx, env, Pause {
            show_menu: flag(c, "showMenu", true),
            topmost: flag(c, "topmost", true),
            local_server: flag(c, "localServer", false),
            friends_available: true,
            reporting_active: true,
        })),
        "death" => {
            let cause = (!c["cause"].is_null()).then(|| t("cause"));
            Box::new(DeathScreen::new(&ctx, env, cause, flag(c, "hardcore", false), int(c, "score", 0), ticks))
        }
        "confirm" => {
            let confirm = Confirm {
                title: t("title"),
                message: t("message"),
                message_style: json_root_style(&c["message"]),
                yes: t("yes"),
                no: t("no"),
                delay: c["delay"].as_u64().map(|d| d as u32),
                ticks,
            };
            let mut s = ConfirmScreen::new(&ctx, env, confirm);
            s.death_background = flag(c, "deathBackground", false);
            Box::new(s)
        }
        "alert" => Box::new(MessageScreen::alert(&ctx, env, t("title"), t("message"), t("ok"))),
        "progress" => {
            let opt = |k: &str| (!c[k].is_null()).then(|| t(k));
            // progressStart sets the stage to "menu.working"; progressStage replaces it.
            let stage = opt("stage").or_else(|| opt("header").map(|_| ctx.tr("menu.working")));
            Box::new(MessageScreen::progress(env, opt("header"), stage, int(c, "progress", 0)))
        }
        "disconnected" => Box::new(MessageScreen::disconnected(&ctx, env, t("title"), t("message"), json_root_style(&c["message"]))),
        "completed" => Box::new(MessageScreen::completed(env, &ctx, t("title"), t("message"), t("button"), ticks)),
        _ => return Vec::new(),
    };
    extract(screen.as_mut(), &mut ctx);
    drop(ctx);
    gui.draws
}
