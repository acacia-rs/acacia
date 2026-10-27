//! The `screens_text` harness: a sign, book or telemetry screen per case; the schema is its
//! `cases.py`'s. The oracle pins blink starts `blinkAgo` ms before its clock, as `MILLIS` is here.

use acacia_ui::java::gui::{Gui, GuiDraw, Metrics};
use acacia_ui::java::screen::book::{BookView, BookViewScreen};
use acacia_ui::java::screen::book_edit::{BookEdit, BookEditScreen};
use acacia_ui::java::screen::sign::{Sign, SignEdit, SignEditScreen};
use acacia_ui::java::screen::telemetry::{Telemetry, TelemetryInfoScreen};
use acacia_ui::java::screen::{Env, Screen, extract};
use acacia_ui::java::widget::Ctx;
use serde_json::{Map, Value};

use super::{flag, int, ints, lang, text};
use crate::Assets;

const MILLIS: i64 = 1_000_000_000;

fn strings(v: &Value) -> Vec<String> {
    v.as_array().into_iter().flatten().map(|s| s.as_str().unwrap_or_default().to_owned()).collect()
}

/// The case's editing ops, as the oracle runs them on the page box's `MultilineTextField`.
fn book_ops(m: &dyn Metrics, book: &mut BookEdit, ops: &Value) {
    let e = &mut book.editor;
    for op in ops.as_array().into_iter().flatten() {
        let n = |i: usize| op[i].as_f64().unwrap_or_default();
        let listener = match op[0].as_str().unwrap_or_default() {
            "seek" => {
                e.field.selecting = op[2].as_bool().unwrap_or_default();
                e.field.seek(n(1) as usize);
                true
            }
            "insert" => e.field.insert_text(m, op[1].as_str().unwrap_or_default()),
            "delete" => e.field.delete_text(m, n(1) as i32),
            "line" => {
                e.field.seek_line(m, n(1) as i32);
                true
            }
            "point" => {
                e.field.seek_point(m, n(1), n(2));
                true
            }
            _ => false,
        };
        if listener {
            e.scroll_to_cursor();
        }
        e.field.selecting = false;
    }
    book.pages[book.page].clone_from(&book.editor.field.value);
}

pub fn replay(c: &Value, assets: &Assets, _: &mut Map<String, Value>) -> Vec<GuiDraw> {
    let lang = lang(assets);
    let tr = |k: &str| lang.get(k).cloned();
    let env = Env { width: int(c, "width", 427), height: int(c, "height", 240), in_world: true, blur: int(c, "blur", 5) >= 1 };
    let mut gui = Gui::new(env.width, env.height);
    let mouse = ints(&c["mouse"]);
    let mut ctx = Ctx::new(&mut gui, assets.font.as_ref(), &tr, (mouse[0], mouse[1]));
    ctx.millis = MILLIS;
    let blink = MILLIS - i64::from(int(c, "blinkAgo", 0));
    let m: &dyn Metrics = assets.font.as_ref();
    let mut screen: Box<dyn Screen> = match c["kind"].as_str().unwrap_or_default() {
        "sign" => {
            let sign = Sign {
                wood: c["wood"].as_str().unwrap_or("oak").to_owned(),
                hanging: flag(c, "hanging", false),
                wall: flag(c, "wall", false),
                dye: c["dye"].as_str().unwrap_or("black").to_owned(),
                glowing: flag(c, "glowing", false),
            };
            let lines = strings(&c["lines"]);
            let mut edit = SignEdit::new(std::array::from_fn(|i| lines.get(i).cloned().unwrap_or_default()), blink);
            edit.line = int(c, "line", 0) as usize;
            edit.cursor = int(c, "cursor", 0) as usize;
            edit.selection = int(c, "selection", 0) as usize;
            Box::new(SignEditScreen::new(&ctx, env, sign, edit))
        }
        "book_view" => {
            let pages = c["pages"].as_array().into_iter().flatten().map(|p| text(p, lang)).collect();
            let mut book = BookView { pages, page: 0 };
            book.set_page(int(c, "page", 0) as usize);
            Box::new(BookViewScreen::new(&ctx, env, &book))
        }
        "book_edit" => {
            let mut book = BookEdit::new(m, strings(&c["pages"]), MILLIS);
            let page = int(c, "page", 0) as usize;
            if page != 0 {
                book.page = page;
                book.load_page(m);
            }
            book_ops(m, &mut book, &c["ops"]);
            book.editor.area.base.focused = flag(c, "focused", true);
            book.editor.focused_time = blink;
            Box::new(BookEditScreen::new(&ctx, env, &book))
        }
        "telemetry" => {
            let t = Telemetry { extra_available: flag(c, "extra", false), opt_in: flag(c, "optIn", false) };
            let mut s = TelemetryInfoScreen::new(&ctx, env, t, c["scroll"].as_f64().unwrap_or_default());
            s.events.area.base.focused = flag(c, "focused", true);
            Box::new(s)
        }
        _ => return Vec::new(),
    };
    extract(screen.as_mut(), &mut ctx);
    drop(ctx);
    gui.draws
}
