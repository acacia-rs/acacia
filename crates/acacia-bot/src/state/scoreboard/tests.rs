use acacia_client::proto::packets::{
    RemoveObjective, SetDisplayObjective, SetScore, SetScoreEntriesItem, SetScoreEntriesItemContent as Content,
    SetScoreEntriesItemContentDefault, SetScoreEntriesItemContentFakePlayer, SetScoreEntriesItemContentRemove,
    SetScoreEntriesItemEntryType as EntryType, SetScoreboardIdentity, SetScoreboardIdentityAction,
    SetScoreboardIdentityEntriesItem,
};

use super::*;
use crate::state::queries::test_support::raw;

fn display(slot: &str, name: &str, sort_order: i32) -> RawPacket {
    raw(&SetDisplayObjective {
        display_slot: slot.into(),
        objective_name: name.into(),
        display_name: "§6§lSKYBLOCK".into(),
        criteria_name: "dummy".into(),
        sort_order,
    })
}

fn fake(id: i64, objective: &str, score: i32, name: &str) -> SetScoreEntriesItem {
    SetScoreEntriesItem {
        entry_type: EntryType::FakePlayer,
        entry_type_name: "changefakeplayer".into(),
        scoreboard_id: id,
        content: Content::FakePlayer(SetScoreEntriesItemContentFakePlayer {
            objective_name: objective.into(),
            score,
            custom_name: name.into(),
        }),
    }
}

fn remove(id: i64, objective: Option<&str>) -> SetScoreEntriesItem {
    SetScoreEntriesItem {
        entry_type: EntryType::Remove,
        entry_type_name: "remove".into(),
        scoreboard_id: id,
        content: Content::Remove(SetScoreEntriesItemContentRemove { objective_name: objective.map(Into::into) }),
    }
}

fn scores(entries: Vec<SetScoreEntriesItem>) -> RawPacket {
    raw(&SetScore { entries })
}

fn sample_sidebar() -> Scoreboard {
    let mut sb = Scoreboard::default();
    sb.apply(&display("sidebar", "sidebar_main", 1)).unwrap();
    sb.apply(&scores(vec![
        fake(1, "sidebar_main", 5, "§a$ §fMoney §a1.5K"),
        fake(2, "sidebar_main", 4, "§d★ §fShards §d120"),
        fake(3, "sidebar_main", 3, "§c🗡 §fKills §c7"),
        fake(4, "sidebar_main", 2, "§e⌚ §fPlaytime §e3d 4h"),
    ]))
    .unwrap();
    sb
}

#[test]
fn sidebar_text_is_sorted_and_stripped() {
    let sb = sample_sidebar();
    let o = sb.sidebar().unwrap();
    assert_eq!(o.display_name, "§6§lSKYBLOCK");
    assert_eq!(o.sort_order, SortOrder::Descending);
    assert_eq!(sb.sidebar_text(), ["$ Money 1.5K", "★ Shards 120", "🗡 Kills 7", "⌚ Playtime 3d 4h"]);
}

#[test]
fn ascending_order_and_name_tiebreak() {
    let mut sb = Scoreboard::default();
    sb.apply(&display("sidebar", "o", 0)).unwrap();
    sb.apply(&scores(vec![fake(1, "o", 2, "b"), fake(2, "o", 1, "z"), fake(3, "o", 2, "a")])).unwrap();
    let lines = sb.sidebar().unwrap().lines();
    assert_eq!(lines, [("z".into(), 1), ("a".into(), 2), ("b".into(), 2)]);
}

#[test]
fn change_replaces_entry_with_same_id() {
    let mut sb = sample_sidebar();
    sb.apply(&scores(vec![fake(1, "sidebar_main", 5, "§a$ §fMoney §a2K")])).unwrap();
    assert_eq!(sb.sidebar_text()[0], "$ Money 2K");
    assert_eq!(sb.sidebar().unwrap().scores.len(), 4);
}

#[test]
fn remove_with_and_without_objective_name() {
    let mut sb = sample_sidebar();
    sb.apply(&display("list", "other", 1)).unwrap();
    sb.apply(&scores(vec![fake(2, "other", 9, "x")])).unwrap();

    sb.apply(&scores(vec![remove(1, Some("sidebar_main"))])).unwrap();
    assert_eq!(sb.sidebar().unwrap().scores.len(), 3);

    sb.apply(&scores(vec![remove(2, None)])).unwrap();
    assert_eq!(sb.sidebar().unwrap().scores.len(), 2);
    assert!(sb.displayed("list").unwrap().scores.is_empty());
}

#[test]
fn player_entries_keep_entity_id() {
    let mut sb = Scoreboard::default();
    sb.apply(&display("belowname", "hp", 1)).unwrap();
    sb.apply(&scores(vec![SetScoreEntriesItem {
        entry_type: EntryType::Player,
        entry_type_name: "changeplayer".into(),
        scoreboard_id: 7,
        content: Content::Default(SetScoreEntriesItemContentDefault {
            objective_name: "hp".into(),
            score: 20,
            entity_unique_id: -42,
        }),
    }]))
    .unwrap();
    let e = &sb.displayed("belowname").unwrap().scores[&7];
    assert_eq!((e.entity_unique_id, e.score, e.display.as_str()), (Some(-42), 20, ""));
}

#[test]
fn remove_objective_clears_slots() {
    let mut sb = sample_sidebar();
    sb.apply(&raw(&RemoveObjective { objective_name: "sidebar_main".into() })).unwrap();
    assert!(sb.sidebar().is_none());
    assert!(sb.objective("sidebar_main").is_none());
    assert!(sb.sidebar_text().is_empty());
}

#[test]
fn redisplay_keeps_scores_and_empty_name_clears_slot() {
    let mut sb = sample_sidebar();
    sb.apply(&display("sidebar", "sidebar_main", 0)).unwrap();
    assert_eq!(sb.sidebar().unwrap().scores.len(), 4);
    sb.apply(&display("sidebar", "", 0)).unwrap();
    assert!(sb.sidebar().is_none());
    assert!(sb.objective("sidebar_main").is_some());
}

#[test]
fn scores_before_display_are_kept() {
    let mut sb = Scoreboard::default();
    sb.apply(&scores(vec![fake(1, "late", 1, "line")])).unwrap();
    sb.apply(&display("sidebar", "late", 1)).unwrap();
    assert_eq!(sb.sidebar_text(), ["line"]);
}

#[test]
fn identities_register_and_clear() {
    let mut sb = Scoreboard::default();
    let identity = |action, uid| {
        raw(&SetScoreboardIdentity {
            action,
            entries: vec![SetScoreboardIdentityEntriesItem { scoreboard_id: 3, entity_unique_id: uid }],
        })
    };
    sb.apply(&identity(SetScoreboardIdentityAction::RegisterIdentity, Some(99))).unwrap();
    assert_eq!(sb.identity(3), Some(99));
    sb.apply(&identity(SetScoreboardIdentityAction::ClearIdentity, None)).unwrap();
    assert_eq!(sb.identity(3), None);
}
