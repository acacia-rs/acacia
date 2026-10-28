use super::confirm::{Confirm, ConfirmScreen, NO};
use super::death::DeathScreen;
use super::options::OptionsScreen;
use super::options::input::{Event, event};
use super::options::pages::{Client, DONE, build};
use super::options::value::V;
use super::pause::{Pause, PauseScreen};
use super::*;
use crate::java::gui::{Fixed, Gui};
use crate::java::text::{Style, Text};
use crate::java::widget::input::{Action, Fired, Interaction, Key};
use crate::java::widget::slider::Slider;

const ENV: Env = Env { width: 427, height: 240, in_world: true, blur: true };
const PAUSE: Pause = Pause { show_menu: true, topmost: true, local_server: false, friends_available: false, reporting_active: true };

fn with_ctx<R>(f: impl FnOnce(&Ctx) -> R) -> R {
    let m = Fixed(6.0);
    let mut gui = Gui::new(ENV.width, ENV.height);
    let lang = |_: &str| -> Option<String> { None };
    let ctx = Ctx::new(&mut gui, &m, &lang, (0, 0));
    f(&ctx)
}

/// The child known by `id`: its index and a point inside it.
fn find(screen: &mut dyn Screen, id: &str) -> (usize, f64, f64) {
    let children = screen.children();
    let (i, c) = children.iter().enumerate().find(|(_, c)| c.widget.base().id == id).unwrap_or_else(|| panic!("no {id}"));
    let b = c.widget.base();
    (i, f64::from(b.x + 1), f64::from(b.y + 1))
}

fn fired(id: &str, action: Action) -> Option<Fired> {
    Some(Fired { id: id.to_owned().into(), action })
}

fn options(page: &str) -> OptionsScreen {
    with_ctx(|ctx| build(ctx, ENV, page, &|_| None, Client::default()).unwrap())
}

#[test]
fn a_click_presses_the_active_button_under_it_and_focuses_it() {
    let mut screen = with_ctx(|ctx| PauseScreen::new(ctx, ENV, PAUSE));
    let mut s = Interaction::default();
    let (i, x, y) = find(&mut screen, "menu.returnToGame");
    assert_eq!(screen.mouse_clicked(&mut s, x, y, false), fired("menu.returnToGame", Action::Press));
    assert_eq!((s.focused, s.dragging), (Some(i), true));
    screen.mouse_released(&mut s);
    assert!(!s.dragging);
    let (_, x, y) = find(&mut screen, "gui.friends.open");
    assert_eq!(screen.mouse_clicked(&mut s, x, y, false), None, "inactive");
    assert_eq!(s.focused, Some(i), "the focus stays");
    let mut rebuilt = with_ctx(|ctx| PauseScreen::new(ctx, ENV, PAUSE));
    rebuilt.restore(&s);
    assert!(rebuilt.children()[i].widget.base().focused);
}

#[test]
fn tab_moves_the_focus_over_active_widgets_and_enter_presses() {
    let mut screen = with_ctx(|ctx| PauseScreen::new(ctx, ENV, PAUSE));
    let mut s = Interaction::default();
    assert_eq!(screen.key_pressed(&mut s, Key::Tab, false), None);
    assert_eq!(s.focused, Some(0));
    assert_eq!(screen.key_pressed(&mut s, Key::Enter, false), fired("menu.returnToGame", Action::Press));
    screen.key_pressed(&mut s, Key::Tab, true);
    assert_eq!(s.focused, Some(find(&mut screen, "menu.disconnect").0), "back past the start, the title skipped");
    assert_eq!(screen.escape(), Escape::Close);
}

#[test]
fn the_death_screen_ignores_escape_and_the_confirm_says_no_once_its_delay_is_over() {
    let death = with_ctx(|ctx| DeathScreen::new(ctx, ENV, None, false, 0, 40));
    assert_eq!(death.escape(), Escape::Ignore);
    let confirm = |ticks| {
        with_ctx(|ctx| {
            let c = Confirm { title: Text::literal("t"), message: Text::default(), message_style: Style::default(), yes: Text::literal("y"), no: Text::literal("n"), delay: Some(20), ticks };
            ConfirmScreen::new(ctx, ENV, c)
        })
    };
    assert_eq!(confirm(5).escape(), Escape::Ignore);
    assert_eq!(confirm(20).escape(), Escape::Fire(Fired { id: NO.into(), action: Action::Press }));
}

#[test]
fn a_slider_follows_the_mouse_from_click_to_release() {
    let mut screen = options("options");
    let mut s = Interaction::default();
    screen.restore(&s);
    let (i, x, y) = find(&mut screen, "fov");
    let left = x - 1.0 + 4.0;
    let slid = screen.mouse_clicked(&mut s, left, y, false).unwrap();
    assert_eq!(slid, fired("fov", Action::Slide(0.0)).unwrap());
    assert_eq!(event(&|_| None, 4, &slid), Event::Set("fov".into(), V::Int(30)));
    assert!(s.can_change && s.focused == Some(i));
    let right = screen.mouse_dragged(&mut s, left + 142.0, y, 0.0).unwrap();
    assert_eq!(event(&|_| None, 4, &right), Event::Set("fov".into(), V::Int(110)));
    screen.mouse_released(&mut s);
    assert_eq!(screen.mouse_dragged(&mut s, left, y, 0.0), None, "no longer dragging");
}

#[test]
fn slider_keys_step_only_while_it_can_change() {
    let mut slider = Slider::new(0, 0, 108, 20, Text::default(), 0.5);
    slider.set_focused(true, false);
    assert_eq!(slider.key_pressed(Key::Right, false), None);
    assert_eq!(slider.key_pressed(Key::Enter, false), Some(None));
    assert_eq!(slider.key_pressed(Key::Right, false), Some(Some(Action::Slide(0.51))));
    slider.set_focused(false, true);
    assert!(!slider.can_change);
}

#[test]
fn option_presses_open_pages_cycle_values_and_slide() {
    let press = |id: &str, action| event(&|_| None, 3, &Fired { id: id.to_owned().into(), action });
    let cycle = |id| press(id, Action::Cycle { back: false });
    assert_eq!(press("options.video", Action::Press), Event::Open("video"));
    assert_eq!(press(DONE, Action::Press), Event::Done);
    assert_eq!(press("options.language", Action::Press), Event::Open("language"));
    assert_eq!(press("credits_and_attribution.button.credits", Action::Press), Event::Open("win"));
    assert_eq!(press("options.online.xboxSettings", Action::Press), Event::Pressed("options.online.xboxSettings".into()));
    assert_eq!(cycle("guiScale"), Event::Set("guiScale".into(), V::Int(1)));
    assert_eq!(press("guiScale", Action::Cycle { back: true }), Event::Set("guiScale".into(), V::Int(3)));
    assert_eq!(cycle("cloudStatus"), Event::Set("cloudStatus".into(), V::Name("OFF".into())), "FANCY wraps to OFF");
    assert_eq!(cycle("enableVsync"), Event::Set("enableVsync".into(), V::Bool(false)));
    let set = event(&|id| (id == "guiScale").then_some(V::Int(9)), 3, &Fired { id: "guiScale".into(), action: Action::Cycle { back: false } });
    assert_eq!(set, Event::Set("guiScale".into(), V::Int(0)), "a scale past the window's starts over");
}

#[test]
fn the_list_scrolls_by_wheel_and_bar_and_clicks_land_on_its_rows_where_drawn() {
    let mut screen = options("video");
    let mut s = Interaction::default();
    screen.restore(&s);
    assert!(screen.mouse_scrolled(&mut s, 213.0, 100.0, -1.0));
    assert_eq!(s.scroll, 12.0);
    assert!(!screen.mouse_scrolled(&mut s, 213.0, 5.0, -1.0), "over the header");
    let mut screen = options("video");
    screen.restore(&s);
    let (_, x, y) = find(&mut screen, "enableVsync");
    assert_eq!(screen.mouse_clicked(&mut s, x, y, false), fired("enableVsync", Action::Cycle { back: false }));
    let bar = f64::from(screen.list.as_ref().unwrap().list.scroll_bar_x()) + 1.0;
    assert_eq!(screen.mouse_clicked(&mut s, bar, 100.0, false), None);
    assert!(s.scrolling && s.focused.is_none());
    screen.mouse_dragged(&mut s, bar, 110.0, 10.0);
    assert!(s.scroll > 12.0, "{}", s.scroll);
    screen.mouse_released(&mut s);
    assert!(!s.scrolling);
    s.scroll = 10_000.0;
    let mut screen = options("video");
    screen.restore(&s);
    let (_, x, y) = find(&mut screen, "framerateLimit");
    assert!(y < 33.0, "scrolled above the list");
    assert_ne!(screen.mouse_clicked(&mut s, x, y, false).map(|f| f.id), Some("framerateLimit".into()));
}
