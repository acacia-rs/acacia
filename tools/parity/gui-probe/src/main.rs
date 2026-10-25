//! `gui-probe <harness> <assets> <inputs.jsonl> <output.jsonl>`: each input line is a case of
//! that harness (`tools/parity/harnesses/<harness>/cases.py`'s schema, its `kind` picking the replay);
//! each output line is `{"draws": [...], <state read back>...}` with the draw calls as
//! `acacia_ui::java::encode` writes them. `assets` is an unpacked jar's `assets/minecraft`. Run by
//! `tools/parity/gui/guiparity.py`.

mod chat;
mod containers;
mod creative;
mod font;
mod hud;
mod kit;
mod lang;
mod players;
mod recipe_book;
mod screens;
mod stations;
mod subtitles;
mod tooltips;
mod trading;

use std::io::{BufWriter, Write};
use std::path::Path;
use std::sync::Arc;

use acacia_ui::java::font::{FontOptions, Fonts};
use acacia_ui::java::gui::Metrics;
use serde_json::{Map, Value, json};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

/// Replays one case; `state` takes what the case reads back besides the draws.
pub type Replay = fn(&Value, &Assets, &mut Map<String, Value>) -> Vec<acacia_ui::java::gui::GuiDraw>;

/// What the probes load once from the assets.
pub struct Assets {
    pub root: std::path::PathBuf,
    /// The client's default font: `fonts` measuring.
    pub font: Box<dyn Metrics>,
    /// Every font the game loads, Unifont from the asset index beside `root` (`<root>/../..`).
    pub fonts: Arc<Fonts>,
}

fn replay(harness: &str) -> Option<Replay> {
    Some(match harness {
        "chat_screen" => chat::replay_screen,
        "containers" => containers::replay,
        "creative" => creative::replay,
        "font" => font::replay,
        "hud" => hud::replay,
        "players" => players::replay,
        "recipe_book" => recipe_book::replay,
        "stations" => stations::replay,
        "screens_widgets" => screens::widgets::replay,
        "screens_menus" => screens::menus::replay,
        "screens_lists" => screens::lists::replay,
        "screens_options" => screens::options::replay,
        "screens_more" => screens::more::replay,
        "subtitles" => subtitles::replay,
        "tooltips" => tooltips::replay,
        "trading" => trading::replay,
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
    let fonts = Arc::new(Fonts::java(&root, root.parent().and_then(Path::parent), FontOptions::default()));
    for w in &fonts.warnings {
        eprintln!("font: {w}");
    }
    let assets = Assets { root, font: Box::new(font::Shared(Arc::clone(&fonts))), fonts };
    let mut out = BufWriter::new(std::fs::File::create(output)?);
    for line in std::fs::read_to_string(inputs)?.lines().filter(|l| !l.is_empty()) {
        let case: Value = serde_json::from_str(line)?;
        let mut state = Map::new();
        let draws = replay(&case, &assets, &mut state);
        if !state.contains_key("draws") {
            state.insert("draws".into(), json!(acacia_ui::java::encode::encode(&draws)));
        }
        writeln!(out, "{}", Value::Object(state))?;
    }
    Ok(())
}
