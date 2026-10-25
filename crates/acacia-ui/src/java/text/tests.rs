use serde_json::json;

use super::*;

fn none(_: &str) -> Option<String> {
    None
}

#[test]
fn formats_like_translatable_contents() {
    let a = [Text::literal("Steve"), Text::literal("Zombie").with_color(0xFF5555)];
    assert_eq!(format_translation("%s was slain by %s", &a).plain(), "Steve was slain by Zombie");
    assert_eq!(format_translation("%2$s then %1$s", &a).plain(), "Zombie then Steve");
    assert_eq!(format_translation("100%% sure %s", &a).plain(), "100% sure Steve");
    assert_eq!(format_translation("%s %s %s", &a).plain(), "%s %s %s");
    assert_eq!(format_translation("%d", &a).plain(), "%d");
    assert_eq!(format_translation("50% off %s", &a).plain(), "50% off Steve");
    assert_eq!(format_translation("%s was slain by %s", &a).spans.last().unwrap().style.color, Some(0xFF5555));
}

#[test]
fn legacy_codes_restyle_and_reset_to_the_span() {
    let t = Text::literal("§cRed §rreset §").with_color(0x5555FF).resolve_legacy();
    assert_eq!(t.plain(), "Red reset ");
    assert_eq!(t.spans[0].style.color, Some(0xFF5555));
    assert_eq!(t.spans[1].style.color, Some(0x5555FF), "§r returns to the component's own colour");
    let t = Text::legacy("§a§lPro§r name");
    assert!(t.spans[0].style.is_bold() && t.spans[0].style.color == Some(0x55FF55));
    assert_eq!(t.spans[1].style, Style::default());
    assert_eq!(Text::literal("§zodd§Rcase").strip_legacy().plain(), "§zoddcase");
}

#[test]
fn components_from_json() {
    let t = Text::from_json(&json!(["Mixed ", {"text": "parts", "color": "gold"}]), &none);
    assert_eq!(t.plain(), "Mixed parts");
    assert_eq!(t.spans[1].style.color, Some(0xFFAA00));
    let t = Text::from_json(&json!({"text": "Boss", "color": "red", "bold": true, "extra": [{"text": "!", "italic": true}]}), &none);
    assert!(t.spans[1].style.is_bold() && t.spans[1].style.is_italic() && t.spans[1].style.color == Some(0xFF5555));
    let lang = |k: &str| (k == "a.b").then(|| "Hi %s".to_owned());
    assert_eq!(Text::from_json(&json!({"translate": "a.b", "with": ["you"]}), &lang).plain(), "Hi you");
    assert_eq!(Text::from_json(&json!({"translate": "x.y", "fallback": "fb"}), &lang).plain(), "fb");
    assert_eq!(Text::from_json(&json!({"translate": "x.y"}), &lang).plain(), "x.y");
    assert_eq!(Text::from_json(&json!({"text": "f", "font": "alt"}), &none).spans[0].style.font.as_deref(), Some("minecraft:alt"));
}
