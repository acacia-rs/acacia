//! The `tooltips` harness's kinds; the case schema is `tools/parity/harnesses/tooltips/cases.py`'s,
//! with `ScreenKit`'s resolved `items` and `TooltipFacts`' `game`.

use serde_json::{Map, Value};

use acacia_ui::java::gui::{Gui, GuiDraw};
use acacia_ui::java::text::Text;
use acacia_ui::java::tooltip::bundle::stack_tooltip;
use acacia_ui::java::tooltip::game::Game;
use acacia_ui::java::tooltip::item::{Details, tooltip_lines};
use acacia_ui::java::tooltip::{Deferred, Line, Positioner, Tooltip};

use crate::Assets;
use crate::kit;

/// The case's `game` with the level facts the tooltip context reads (`maps`, `peaceful`).
fn game(c: &Value) -> Game {
    let mut game = c["game"].clone();
    if let Value::Object(g) = &mut game {
        g.insert("maps".to_owned(), c["maps"].clone());
        g.insert("peaceful".to_owned(), Value::Bool(kit::bool(c, "peaceful")));
    }
    Game::new(game)
}

fn positioner(t: &Value) -> Positioner {
    let p = &t["positioner"];
    let rect = [1, 2, 3, 4].map(|i| p[i].as_i64().unwrap_or(0) as i32);
    match p[0].as_str() {
        Some("below") => Positioner::BelowOrAbove(rect),
        Some("menu") => Positioner::Menu(rect),
        _ => Positioner::Default,
    }
}

pub fn replay(c: &Value, assets: &Assets, state: &mut Map<String, Value>) -> Vec<GuiDraw> {
    let lang = kit::lang(assets);
    let font = assets.font.as_ref();
    let mut gui = Gui::new(kit::int(c, "width", 0), kit::int(c, "height", 0));
    let mut deferred = Deferred::default();
    if c["kind"] == "lines" {
        for t in c["tooltips"].as_array().into_iter().flatten() {
            let lines = t["lines"].as_array().into_iter().flatten().map(|l| Line::Text(Text::from_json(l, &lang))).collect();
            let style = t["style"].as_str().map(str::to_owned);
            let tooltip = Tooltip { lines, x: kit::int(t, "x", 0), y: kit::int(t, "y", 0), positioner: positioner(t), style, extra_space: false };
            deferred.set(tooltip, kit::bool(t, "replace"));
        }
    } else {
        let game = game(c);
        let d = Details {
            lang: &lang,
            advanced: kit::bool(c, "advanced"),
            creative: kit::bool(c, "creative"),
            game: &game,
            player: Some(game.player(kit::bool(c, "op"))),
            key: Some(0),
        };
        let stack = kit::item(c, 0).unwrap_or_default();
        let plain = if stack.is_empty() { Vec::new() } else { tooltip_lines(&stack, &d) };
        state.insert("lines".to_owned(), plain.iter().map(|l| Value::from(l.plain())).collect());
        if !stack.is_empty() {
            deferred.set(stack_tooltip(&stack, &d, font, kit::mouse(c)), false);
        }
    }
    deferred.extract(&mut gui, font);
    gui.draws
}
