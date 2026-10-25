use super::*;
use crate::control::Stack;

fn stack(name: &str) -> Stack {
    Stack { name: name.into(), aux: 0, count: 1, block: 0, enchanted: false, dye: None, banner: None, map: None, title: None, enchantments: Vec::new(), lore: Vec::new(), trim: None }
}

fn me() -> Me {
    Me {
        eye: glam::DVec3::ZERO,
        on_ground: true,
        sprinting: false,
        flying: false,
        riding: false,
        carrying_yaw: None,
        hurts: 0,
        armor: 7,
        air: None,
        mining: None,
        hotbar: 2,
        game_mode: GameMode::Survival,
        alive: true,
        rain: 0.0,
        thunder: 0.0,
        bolts: Vec::new(),
        health: 15.0,
        max_health: 20.0,
        food: 18.0,
        saturation: 2.5,
        head: None,
        xp_level: 4,
        xp_progress: 0.25,
        items: std::array::from_fn(|i| (i == 2).then(|| stack("minecraft:stone"))),
        book: None,
        offhand: None,
        effects: vec![(19, 100, 0, false)],
        bosses: vec![("entity.wither.name".into(), 0.5, 5)],
    }
}

fn view<'a>(me: &'a Me, lang: &'a Lang, effects: &'a [Active]) -> HudView<'a> {
    HudView { me, effects, screen: None, chat_open: false, debug: false, sidebar: None, players: None, lang }
}

#[test]
fn player_state_maps_to_java_fields() {
    let (me, lang) = (me(), Lang::default());
    let i = inputs(&view(&me, &lang, &[]), [320, 240], 1025, 1.0);
    assert_eq!(i.partial, 0.5);
    assert_eq!(i.game_mode, GameType::Survival);
    assert_eq!(i.hotbar.selected, 2);
    assert_eq!(i.hotbar.slots.iter().filter(|s| s.is_some()).count(), 1);
    assert_eq!((i.health.armor, i.health.food, i.health.saturation), (7, 18, 2.5));
    assert_eq!(i.health.kind, HeartKind::Poisoned);
    assert_eq!((i.health.air, i.health.under_water), (300, false));
    assert_eq!(i.bosses[0].color, 5, "BOSS_COLOURS and Java's BossBarColor share their order");
    assert!(i.camera.equipment.is_empty());
}

#[test]
fn screens_as_the_hud_sees_them() {
    assert_eq!(screen_info(None, false, false, 320), None);
    assert_eq!(screen_info(Some(Layout::Player), true, true, 320), Some(ScreenInfo::default()), "a menu over it");
    assert!(screen_info(Some(Layout::Player), false, false, 320).unwrap().shows_effects);
    assert!(!screen_info(Some(Layout::Player), false, false, 200).unwrap().shows_effects, "no room beside it");
    assert!(!screen_info(Some(Layout::Rows(3)), false, false, 320).unwrap().shows_effects);
    assert!(screen_info(None, true, false, 320).unwrap().in_game_ui);
}

#[test]
fn game_modes() {
    assert_eq!(game_type(GameMode::CreativeSpectator), GameType::Creative);
    assert_eq!(game_type(GameMode::SurvivalSpectator), GameType::Spectator);
    assert_eq!(game_type(GameMode::Fallback), GameType::Survival);
}

#[test]
fn bedrock_effects_become_java_icons() {
    let poison = effect_icon(&Active { id: 25, ticks: 40, ambient: true }).unwrap();
    assert_eq!((poison.id.as_str(), poison.beneficial, poison.duration, poison.ambient), ("poison", false, 40, true));
    assert_eq!(effect_icon(&Active { id: 1, ticks: -1, ambient: false }).unwrap().id, "speed");
    assert!(effect_icon(&Active { id: 6, ticks: 1, ambient: false }).is_none(), "instant health has no icon");
}

#[test]
fn held_name_takes_the_anvil_name_and_enchanted_rarity() {
    let mut me = me();
    let lang = Lang::default();
    let held = held_name(&me, &lang).unwrap();
    assert_eq!((held.hover_name.plain(), held.rarity_color, held.custom_name), ("Stone".to_owned(), COMMON, false));
    let s = me.items[2].as_mut().unwrap();
    s.title = Some("Rock".into());
    s.enchanted = true;
    let held = held_name(&me, &lang).unwrap();
    assert_eq!((held.hover_name.plain(), held.rarity_color, held.custom_name), ("Rock".to_owned(), RARE, true));
    me.hotbar = 0;
    assert!(held_name(&me, &lang).is_none());
}

#[test]
fn screens_chat_debug_sidebar_players_and_pumpkin() {
    let mut me = me();
    me.head = Some("minecraft:carved_pumpkin".into());
    let lang = Lang::default();
    let sidebar = Sidebar { title: "Kills".into(), lines: vec![("bob".into(), 3)] };
    let names = ["Alex".to_owned(), "Steve".to_owned()];
    let screen = Some(ScreenInfo { in_game_ui: true, ..ScreenInfo::default() });
    let v = HudView { screen, chat_open: true, debug: true, sidebar: Some(&sidebar), players: Some(&names), ..view(&me, &lang, &[]) };
    let i = inputs(&v, [320, 240], 0, 0.0);
    assert_eq!(i.screen, screen);
    assert!(i.chat_focused && i.crosshair_3d && i.tab.key_down);
    assert_eq!(i.tab.players.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), ["Alex", "Steve"]);
    let objective = i.sidebar.unwrap();
    assert_eq!((objective.title.plain(), objective.scores[0].owner.as_str(), objective.scores[0].value), ("Kills".to_owned(), "bob", 3));
    assert_eq!(i.camera.equipment, ["misc/pumpkinblur"]);
}
