use acacia_client::proto::packets::ClientCameraAimAssistAction;

use super::{clear_aim_assist, Reflex, Reflexes};
use crate::interact::SwingSource;
use crate::state::queries::test_support::raw;

#[test]
fn reflexes_fire_once_after_their_ticks_in_order() {
    let mut r = Reflexes::default();
    r.schedule(2, Reflex::StopUseOn([1, 2, 3]));
    r.schedule(0, Reflex::Swing(SwingSource::Interact));
    r.schedule(0, Reflex::Equip);
    r.schedule(5, Reflex::Equip);
    assert_eq!(r.tick(), [Reflex::Swing(SwingSource::Interact), Reflex::Equip], "a waiting reflex is not doubled");
    assert!(r.tick().is_empty());
    assert_eq!(r.tick(), [Reflex::StopUseOn([1, 2, 3])]);
    assert!(r.tick().is_empty());
    assert!(!r.is_scheduled(Reflex::Equip));
}

#[test]
fn legacy_ids_are_negative_even_and_count_down() {
    let mut r = Reflexes::default();
    let ids: Vec<i32> = (0..3).map(|_| r.next_legacy_id()).collect();
    assert_eq!(ids, [-2, -4, -6]);
}

#[test]
fn aim_assist_clear_matches_vanilla() {
    let p = clear_aim_assist();
    assert_eq!((p.preset_id.as_str(), p.action, p.allow_aim_assist), ("", ClientCameraAimAssistAction::Clear, false));
    assert_eq!(raw(&p).decode::<acacia_client::proto::packets::ClientCameraAimAssist>().unwrap(), p);
}
