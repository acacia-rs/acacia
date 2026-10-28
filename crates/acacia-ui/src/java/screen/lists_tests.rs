use super::keys::{Category, Key, Os, defaults, sorted};
use super::language::{Language, filtered};
use super::packs::{self, Compat, Pack};
use crate::java::text::{Style, Text};

fn lang(k: &str) -> Option<String> {
    match k {
        "key.keyboard.left.meta" => Some("Left Meta".into()),
        "key.keyboard.left.windows" => Some("Left Windows".into()),
        "key.mouse.left" => Some("Left Button".into()),
        "key.mouse" => Some("Button %1$s".into()),
        _ => None,
    }
}

#[test]
fn key_names_follow_input_constants() {
    assert_eq!(Key::keyboard(26).name(), "key.keyboard.w");
    assert_eq!(Key::keyboard(300).name(), "key.keyboard.300");
    assert_eq!(Key::mouse(3).name(), "key.mouse.right");
    assert_eq!(Key::keyboard(26).display(&lang, Os::Linux).plain(), "W");
    assert_eq!(Key::keyboard(56).display(&lang, Os::Linux).plain(), "/");
    assert_eq!(Key::keyboard(227).display(&lang, Os::Linux).plain(), "Left Meta");
    assert_eq!(Key::keyboard(227).display(&lang, Os::Windows).plain(), "Left Windows");
    assert_eq!(Key::keyboard(44).display(&lang, Os::Linux).plain(), "key.keyboard.space");
    assert_eq!(Key::mouse(1).display(&lang, Os::Linux).plain(), "Left Button");
    assert_eq!(Key::mouse(4).display(&lang, Os::Linux).plain(), "Button 4");
}

#[test]
fn mappings_sort_by_category_order_then_name() {
    let s = sorted(&defaults(), &|_| None);
    let movement: Vec<_> = s.iter().take_while(|m| m.category == Category::Movement).map(|m| m.name).collect();
    assert_eq!(movement, ["key.back", "key.forward", "key.jump", "key.left", "key.right", "key.sneak", "key.sprint"]);
    let debug: Vec<_> = s.iter().filter(|m| m.category == Category::Debug).map(|m| m.name).collect();
    assert_eq!(&debug[..2], ["key.debug.overlay", "key.debug.modifier"]);
    assert_eq!(&debug[debug.len() - 4..], ["key.debug.profilingChart", "key.debug.fpsCharts", "key.debug.networkCharts", "key.debug.lightmapTexture"]);
    assert_eq!(s.len(), defaults().len());
}

fn language(code: &str, name: &str, region: &str) -> Language {
    Language { code: code.into(), name: name.into(), region: region.into() }
}

#[test]
fn languages_sort_by_code_and_filter_by_name_or_region() {
    let all = [language("fr_fr", "Francais", "France"), language("en_us", "English", "US"), language("en_gb", "English", "UK")];
    let codes = |v: Vec<&Language>| v.iter().map(|l| l.code.clone()).collect::<Vec<_>>();
    assert_eq!(codes(filtered(&all, "")), ["en_gb", "en_us", "fr_fr"]);
    assert_eq!(codes(filtered(&all, "uk")), ["en_gb"]);
    assert_eq!(codes(filtered(&all, "FRAN")), ["fr_fr"]);
}

#[test]
fn packs_filter_by_id_title_or_description() {
    let pack = |id: &str, title: &str, d: &str| Pack {
        id: id.into(),
        title: Text::literal(title),
        description: Text::literal(d),
        description_style: Style::default(),
        icon: "misc/unknown_pack".into(),
        compat: Compat::Compatible,
        selected: false,
        required: false,
        fixed: false,
        can_move_up: false,
        can_move_down: false,
    };
    let all = [pack("vanilla", "Default", "The default look"), pack("file/x.zip", "Faithful", "32x")];
    let ids = |v: Vec<&Pack>| v.iter().map(|p| p.id.clone()).collect::<Vec<_>>();
    assert_eq!(ids(packs::filtered(&all, "  ")), ["vanilla", "file/x.zip"]);
    assert_eq!(ids(packs::filtered(&all, "LOOK")), ["vanilla"]);
    assert_eq!(ids(packs::filtered(&all, "zip")), ["file/x.zip"]);
}
