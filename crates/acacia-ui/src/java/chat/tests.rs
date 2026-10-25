use super::component::time_alpha;
use super::*;
use crate::java::encode::encode;
use crate::java::gui::{Fixed, Gui};
use crate::java::text::Text;

#[test]
fn lines_fade_out_over_the_last_20_of_200_ticks() {
    assert_eq!(time_alpha(1000, 1000), 1.0);
    assert_eq!(time_alpha(1180, 1000), 1.0);
    assert!((time_alpha(1190, 1000) - 0.25).abs() < 1e-6);
    assert_eq!(time_alpha(1200, 1000), 0.0);
    // A message from the future is opaque.
    assert_eq!(time_alpha(1000, 1005), 1.0);
}

#[test]
fn messages_wrap_newest_at_the_bottom_with_indicators() {
    let m = Fixed(6.0);
    let mut chat = Chat::new(ChatOptions { width: 0.0, ..ChatOptions::default() });
    // 40 px wide: "aaa bbb" breaks after "aaa".
    chat.add(&m, Text::literal("aaa bbb"), None, 100, false);
    chat.add(&m, Text::literal("c"), Some(Tag::SYSTEM), 100, false);
    let mut gui = Gui::new(320, 240);
    chat.extract(&mut gui, 100, false);
    let v = encode(&gui.draws);
    let texts: Vec<(&str, i64)> = v.iter().filter(|d| d["type"] == "text").map(|d| (d["text"].as_str().unwrap(), d["y"].as_i64().unwrap())).collect();
    assert_eq!(texts, [("aaa", 200 - 18 - 8), (" bbb", 200 - 9 - 8), ("c", 200 - 8)]);
    let last = &v[v.len() - 1];
    assert_eq!((last["x0"].as_i64(), last["x1"].as_i64(), last["color"].as_i64()), (Some(-4), Some(-2), Some(0xFFD0D0D0u32 as i32 as i64)));
}

#[test]
fn scrolling_stops_at_the_last_page() {
    let m = Fixed(6.0);
    let mut chat = Chat::default();
    for n in 0..30 {
        chat.add(&m, Text::literal(format!("{n}")), None, 0, false);
    }
    // Focused: 180 px, 20 lines a page.
    chat.scroll_chat(1000, true);
    let mut gui = Gui::new(320, 240);
    chat.extract(&mut gui, 0, true);
    let texts: Vec<_> = encode(&gui.draws).into_iter().filter(|d| d["type"] == "text").map(|d| d["text"].as_str().unwrap().to_owned()).collect();
    assert_eq!(texts.len(), 20);
    assert_eq!((texts[0].as_str(), texts[19].as_str()), ("0", "19"));
}
