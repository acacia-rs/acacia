use acacia_client::proto::types::Vec3f;

use super::*;
use crate::state::queries::test_support::raw;

fn level_event(event: LevelEventEvent, data: i32) -> RawPacket {
    raw(&LevelEvent { event, position: Vec3f { x: 0.0, y: 0.0, z: 0.0 }, data })
}

fn boss(r#type: BossEventType, title: &str, progress: f32) -> RawPacket {
    raw(&BossEvent {
        target_entity_id: -9,
        r#type,
        title: title.into(),
        filtered_title: String::new(),
        progress,
        color: BossEventColor::Pink,
        overlay: BossEventOverlay::Progress,
    })
}

#[test]
fn weather_follows_level_events_and_ignores_the_rest() {
    let mut env = Environment::default();
    env.apply(&level_event(LevelEventEvent::StartRain, 32768)).unwrap();
    assert!(env.is_raining() && (env.rain - 0.5).abs() < 0.01);
    env.apply(&level_event(LevelEventEvent::StartThunder, 0)).unwrap();
    assert!(env.is_thundering(), "a zero intensity still starts it");
    env.apply(&level_event(LevelEventEvent::SoundClick, 0)).unwrap();
    env.apply(&level_event(LevelEventEvent::StopRain, 0)).unwrap();
    env.apply(&level_event(LevelEventEvent::StopThunder, 0)).unwrap();
    assert!(!env.is_raining() && !env.is_thundering());
    env.apply(&raw(&SetTime { time: 6000 })).unwrap();
    assert_eq!(env.time, 6000);
}

#[test]
fn boss_bars_show_update_and_hide() {
    let mut env = Environment::default();
    env.apply(&boss(BossEventType::ShowBar, "Wither", 1.0)).unwrap();
    env.apply(&boss(BossEventType::SetBarProgress, "", 0.25)).unwrap();
    env.apply(&boss(BossEventType::SetBarTitle, "Wither II", 0.0)).unwrap();
    assert_eq!(env.boss_bars[&-9], BossBar {
        title: "Wither II".into(),
        progress: 0.25,
        color: BossEventColor::Pink,
        overlay: BossEventOverlay::Progress
    });
    env.apply(&boss(BossEventType::HideBar, "", 0.0)).unwrap();
    assert!(env.boss_bars.is_empty());
}
