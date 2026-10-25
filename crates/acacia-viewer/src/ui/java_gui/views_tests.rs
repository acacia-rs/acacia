use std::path::Path;
use std::time::Instant;

use acacia_ui::java::widget::input::Interaction;

use super::JavaGui;
use super::menus::View;
use crate::settings::Settings;

fn calls(view: View) -> usize {
    let gui = JavaGui::new(Path::new("no-such-look"));
    gui.menu(&view, &Interaction::default(), [427, 240], (0, 0), Instant::now()).draws.len()
}

#[test]
fn every_screen_beside_the_options_opens_and_draws() {
    for page in ["credits", "keybinds", "language", "packs", "telemetry"] {
        assert!(calls(View::Options { page, settings: Settings::default() }) > 4, "{page}");
    }
    let sign = View::Sign { lines: ["a".into(), "bc".into(), String::new(), String::new()], line: 1, cursor: 1, hanging: false };
    let pages = vec!["one".to_owned(), "two".to_owned()];
    for view in [sign, View::Book { pages: pages.clone(), page: 1, writing: false, caret: 0 }, View::Book { pages, page: 5, writing: true, caret: 99 }, View::Loading] {
        assert!(calls(view.clone()) > 2, "{view:?}");
    }
}

#[test]
fn the_credits_roll_scrolls_with_time_and_ends() {
    let gui = JavaGui::new(Path::new("no-such-look"));
    let roll = super::views::credits_roll(&gui, [427, 240], 10.5);
    assert_eq!((roll.scroll, roll.partial), (7.5, 0.5));
    assert!(super::views::credits_roll(&gui, [427, 240], 1000.0).finished(), "no credits.json: nothing to roll");
}
