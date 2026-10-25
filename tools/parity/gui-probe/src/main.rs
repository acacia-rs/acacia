//! `gui-probe <harness> <assets> <inputs.jsonl> <output.jsonl>`: each input line is a case of
//! that harness (`tools/parity/harnesses/<harness>/cases.py`'s schema, its `kind` picking the replay);
//! each output line is `{"draws": [...], <state read back>...}` with the draw calls as
//! `acacia_ui::java::encode` writes them. `assets` is an unpacked jar's `assets/minecraft`. Run by
//! `tools/parity/gui/guiparity.py`.

mod hud;
mod screens;

use std::io::{BufWriter, Write};
use std::path::Path;

use acacia_ui::java::gui::Metrics;
use acacia_ui::java::text::Style;
use serde_json::{Map, Value, json};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

/// Replays one case; `state` takes what the case reads back besides the draws.
pub type Replay = fn(&Value, &Assets, &mut Map<String, Value>) -> Vec<acacia_ui::java::gui::GuiDraw>;

/// What the probes load once from the assets.
pub struct Assets {
    pub root: std::path::PathBuf,
    pub font: Box<dyn Metrics>,
}

// TODO: the Java look's font once it has every provider; the ASCII page until then.
struct Ascii(Option<acacia_ui::font::Font>);

impl Metrics for Ascii {
    fn advance(&self, c: char, style: &Style) -> f32 {
        self.0.as_ref().map_or(0.0, |f| f.width(&c.to_string())) + f32::from(u8::from(style.is_bold()))
    }
}

fn replay(harness: &str) -> Option<Replay> {
    Some(match harness {
        "hud" => hud::replay,
        "screens_widgets" => screens::widgets::replay,
        "screens_menus" => screens::menus::replay,
        _ => return None,
    })
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [harness, root, inputs, output] = args.as_slice() else {
        return Err("usage: gui-probe HARNESS ASSETS INPUTS OUTPUT".into());
    };
    let replay = replay(harness).ok_or_else(|| format!("unknown harness {harness}"))?;
    let root = Path::new(root).to_owned();
    let font = Ascii(acacia_ui::theme::java_font(&root, &mut acacia_ui::Atlas::new()));
    let assets = Assets { root, font: Box::new(font) };
    let mut out = BufWriter::new(std::fs::File::create(output)?);
    for line in std::fs::read_to_string(inputs)?.lines().filter(|l| !l.is_empty()) {
        let case: Value = serde_json::from_str(line)?;
        let mut state = Map::new();
        let draws = replay(&case, &assets, &mut state);
        state.insert("draws".into(), json!(acacia_ui::java::encode::encode(&draws)));
        writeln!(out, "{}", Value::Object(state))?;
    }
    Ok(())
}
