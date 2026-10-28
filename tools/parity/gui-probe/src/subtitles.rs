//! The `subtitles` harness: replays a script (`tools/parity/harnesses/subtitles/cases.py`) through
//! `SubtitleOverlay` and keeps the last frame's draws.

use serde_json::{Map, Value};

use acacia_ui::java::gui::{Gui, GuiDraw};
use acacia_ui::java::hud::subtitles::{SubtitleFrame, SubtitleOverlay};
use acacia_ui::java::hud::titles::background_color;

use crate::Assets;
use crate::lang::translate;

fn vec3(v: &Value) -> [f64; 3] {
    std::array::from_fn(|i| v[i].as_f64().unwrap_or(0.0))
}

pub fn replay(c: &Value, assets: &Assets, _: &mut Map<String, Value>) -> Vec<GuiDraw> {
    let mut overlay = SubtitleOverlay::default();
    let mut now = 0i64;
    let mut gui = Gui::default();
    for op in c["ops"].as_array().into_iter().flatten() {
        match op["op"].as_str().unwrap_or_default() {
            "time" => now = op["ms"].as_i64().unwrap_or(0),
            "play" if overlay.listening() => {
                let range = op["range"].as_f64().map_or(f32::INFINITY, |r| r as f32);
                let key = op["key"].as_str().map(|k| (k.to_owned(), translate(assets, k)));
                overlay.on_play(key, vec3(&op["at"]), range, op["looping"].as_bool().unwrap_or(false), now);
            }
            "play" => {}
            "frame" => {
                let frame = SubtitleFrame {
                    show: op["show"].as_bool().unwrap_or(false),
                    position: vec3(&op["pos"]),
                    forward: vec3(&op["forward"]),
                    up: vec3(&op["up"]),
                    multiplier: op["mult"].as_f64().unwrap_or(1.0),
                    background: background_color(op["chatOnly"].as_bool().unwrap_or(true), op["opacity"].as_f64().unwrap_or(0.5) as f32, 0.8),
                };
                gui = Gui::new(op["w"].as_i64().unwrap_or(0) as i32, op["h"].as_i64().unwrap_or(0) as i32);
                overlay.extract(&mut gui, assets.font.as_ref(), &frame, now);
            }
            other => panic!("unknown subtitle op {other}"),
        }
    }
    gui.draws
}
