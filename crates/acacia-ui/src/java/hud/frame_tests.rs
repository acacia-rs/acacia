use super::*;
use crate::java::gui::{Fixed, GuiDraw};

fn inputs() -> HudInputs {
    HudInputs {
        width: 320,
        height: 240,
        first_person: true,
        attack_scale: 1.0,
        health: Health { health: 20.0, max_health: 20.0, food: 20, saturation: 5.0, air: 300, max_air: 300, ..Health::default() },
        ..HudInputs::default()
    }
}

fn run(hud: &mut Hud, i: &HudInputs) -> Gui {
    let mut gui = Gui::new(i.width, i.height);
    hud.extract(&mut gui, &Fixed(6.0), &|_| None, i);
    gui
}

fn sprites(gui: &Gui) -> Vec<String> {
    gui.draws
        .iter()
        .filter_map(|d| match d {
            GuiDraw::Sprite { sprite, .. } => Some(sprite.to_string()),
            _ => None,
        })
        .collect()
}

fn texts(gui: &Gui) -> Vec<String> {
    gui.draws
        .iter()
        .filter_map(|d| match d {
            GuiDraw::Text { text, .. } => Some(text.plain()),
            _ => None,
        })
        .collect()
}

fn first(names: &[String], prefix: &str) -> usize {
    names.iter().position(|n| n.starts_with(prefix)).unwrap_or_else(|| panic!("no {prefix} in {names:?}"))
}

fn player(name: &str) -> TabPlayer {
    TabPlayer { uuid: 1, name: name.into(), game_mode: 0, latency: 0, display_name: None, list_order: 0, show_hat: true, skin: String::new(), upside_down: false }
}

#[test]
fn pieces_in_the_games_order() {
    let mut hud = Hud::default();
    hud.chat.add(&Fixed(6.0), Text::literal("hi"), None, 0, false);
    let mut i = inputs();
    i.effects = vec![EffectIcon::vanilla("speed", 400, false, true).unwrap()];
    i.bosses = vec![BossBar { name: Text::literal("Boss"), progress: 0.5, color: 0, overlay: 0 }];
    i.tab = TabFrame { key_down: true, players: vec![player("Alex")], ..TabFrame::default() };
    let gui = run(&mut hud, &i);
    assert_eq!(gui.draws[0], GuiDraw::Stratum, "the crosshair's stratum");
    assert!(matches!(&gui.draws[2], GuiDraw::Stratum), "then the hotbar's");
    let names = sprites(&gui);
    let order = ["hud/crosshair", "hud/hotbar", "hud/heart/container", "hud/food_empty", "hud/experience_bar_background", "hud/effect_background", "boss_bar/", "icon/ping"];
    let at: Vec<usize> = order.iter().map(|p| first(&names, p)).collect();
    assert!(at.windows(2).all(|w| w[0] < w[1]), "{at:?}");
    assert_eq!(texts(&gui), ["Boss", "hi", "Alex"]);
}

#[test]
fn hidden_draws_only_the_sleep_overlay() {
    let mut i = inputs();
    i.hidden = true;
    i.sleep_timer = 50;
    let gui = run(&mut Hud::default(), &i);
    assert!(matches!(gui.draws[..], [GuiDraw::Stratum, GuiDraw::Fill { .. }]));
}

#[test]
fn spectators_get_no_crosshair_hotbar_or_health() {
    let mut i = inputs();
    i.game_mode = GameType::Spectator;
    i.xp_level = 5;
    assert!(sprites(&run(&mut Hud::default(), &i)).is_empty());
    i.spectator_crosshair = true;
    assert_eq!(sprites(&run(&mut Hud::default(), &i)), ["hud/crosshair"]);
}

#[test]
fn creative_has_hotbar_but_no_health_or_experience() {
    let mut i = inputs();
    i.game_mode = GameType::Creative;
    i.xp_level = 5;
    i.xp_progress = 0.5;
    let names = sprites(&run(&mut Hud::default(), &i));
    assert!(names.iter().all(|n| !n.starts_with("hud/heart") && !n.starts_with("hud/experience")), "{names:?}");
    assert!(names.contains(&"hud/hotbar".to_owned()));
}

#[test]
fn screens_hide_what_they_show_themselves() {
    let mut hud = Hud::default();
    hud.chat.add(&Fixed(6.0), Text::literal("hi"), None, 0, false);
    let mut i = inputs();
    i.effects = vec![EffectIcon::vanilla("speed", 400, false, true).unwrap()];
    i.screen = Some(ScreenInfo { shows_effects: true, in_game_ui: true, level_loading: false });
    i.chat_focused = true;
    let gui = run(&mut hud, &i);
    assert!(!sprites(&gui).iter().any(|n| n.starts_with("hud/effect")));
    assert!(texts(&gui).is_empty());
    i.screen = Some(ScreenInfo { level_loading: true, ..ScreenInfo::default() });
    assert!(run(&mut hud, &i).draws.is_empty());
}

#[test]
fn the_3d_crosshair_keeps_the_hotbar_stratum() {
    let mut i = inputs();
    i.crosshair_3d = true;
    let gui = run(&mut Hud::default(), &i);
    assert_eq!(gui.draws[0], GuiDraw::Stratum);
    assert!(matches!(&gui.draws[1], GuiDraw::Sprite { sprite, .. } if sprite == "hud/hotbar"));
}

#[test]
fn attack_indicator_goes_where_the_option_says() {
    let mut i = inputs();
    i.attack_scale = 0.5;
    assert!(sprites(&run(&mut Hud::default(), &i)).contains(&"hud/crosshair_attack_indicator_background".to_owned()));
    i.attack_indicator = AttackIndicator::Hotbar;
    let names = sprites(&run(&mut Hud::default(), &i));
    assert!(names.contains(&"hud/hotbar_attack_indicator_background".to_owned()));
    assert!(!names.iter().any(|n| n.starts_with("hud/crosshair_attack")));
}

#[test]
fn jump_bar_replaces_the_experience_bar() {
    let mut i = inputs();
    i.jump = Some((0, 0.5));
    i.xp_level = 3;
    let gui = run(&mut Hud::default(), &i);
    let names = sprites(&gui);
    assert!(names.contains(&"hud/jump_bar_background".to_owned()));
    assert!(!names.iter().any(|n| n.starts_with("hud/experience")));
    assert_eq!(texts(&gui).last().map(String::as_str), Some("3"));
}

#[test]
fn vehicle_hearts_replace_food() {
    let mut i = inputs();
    i.vehicle = Some(Vehicle { max_health: 30.0, health: 30.0 });
    let names = sprites(&run(&mut Hud::default(), &i));
    assert!(!names.iter().any(|n| n.starts_with("hud/food")));
    assert_eq!(names.iter().filter(|n| *n == "hud/heart/vehicle_full").count(), 15);
}

#[test]
fn tab_list_hidden_alone_on_a_local_server() {
    let mut i = inputs();
    i.tab = TabFrame { key_down: true, local_server: true, players: vec![player("Alex")], ..TabFrame::default() };
    assert!(!sprites(&run(&mut Hud::default(), &i)).iter().any(|n| n.starts_with("icon/ping")));
    i.tab.local_server = false;
    assert!(sprites(&run(&mut Hud::default(), &i)).iter().any(|n| n.starts_with("icon/ping")));
}

#[test]
fn subtitles_wait_for_the_screen_unless_one_is_open_outside_the_game() {
    let mut hud = Hud::default();
    hud.subtitles.on_play(Some(("a".into(), Text::literal("Door"))), [1.0, 0.0, 0.0], 16.0, false, 0);
    let mut i = inputs();
    i.subtitles = SubtitleFrame { show: true, forward: [0.0, 0.0, 1.0], up: [0.0, 1.0, 0.0], multiplier: 1.0, ..SubtitleFrame::default() };
    let mut gui = run(&mut hud, &i);
    assert!(!texts(&gui).contains(&"Door".to_owned()));
    hud.extract_deferred_subtitles(&mut gui, &Fixed(6.0));
    assert!(texts(&gui).contains(&"Door".to_owned()));
    i.screen = Some(ScreenInfo::default());
    assert!(texts(&run(&mut hud, &i)).contains(&"Door".to_owned()), "a chat screen: drawn in place");
    let mut gui = Gui::new(320, 240);
    hud.extract_deferred_subtitles(&mut gui, &Fixed(6.0));
    assert!(gui.draws.is_empty());
    i.hidden = true;
    i.screen = Some(ScreenInfo { in_game_ui: true, ..ScreenInfo::default() });
    run(&mut hud, &i);
    hud.extract_deferred_subtitles(&mut gui, &Fixed(6.0));
    assert!(texts(&gui).contains(&"Door".to_owned()), "hidden, but over an in-game screen");
}

#[test]
fn team_colour_objective_wins_the_sidebar() {
    let objective = |t: &str| SidebarObjective { title: Text::literal(t), format: None, scores: Vec::new() };
    let mut i = inputs();
    i.sidebar = Some(objective("Plain"));
    i.team_sidebar = Some(objective("Team"));
    assert!(texts(&run(&mut Hud::default(), &i)).contains(&"Team".to_owned()));
}

#[test]
fn ticks_count_down_titles_and_the_held_name() {
    let mut hud = Hud::default();
    hud.titles.set_title(Text::literal("T"));
    let held = HeldName { item: "stone".into(), hover_name: Text::literal("Stone"), rarity_color: 0xFF_FFFF, custom_name: false };
    hud.tick(Some(held.clone()), 1.0);
    hud.tick(Some(held), 1.0);
    assert_eq!((hud.tick_count, hud.titles.time, hud.highlight.timer), (2, 98, 39));
}

#[test]
fn effects_fit_beside_a_centred_container() {
    assert!(container_shows_effects(320, 176));
    assert!(!container_shows_effects(200, 176));
}
