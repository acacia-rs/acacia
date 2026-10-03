use acacia_client::proto::types::{PlayerRecord, PlayerRecordContentRemove, PlayerRecordType};

use super::*;
use crate::state::queries::test_support::{fixtures, raw};

fn template() -> PlayerRecord {
    fixtures::<PlayerListPacket>()
        .iter()
        .flat_map(|raw| raw.decode::<PlayerListPacket>().unwrap().records)
        .find(|r| matches!(r.content, PlayerRecordContent::Add(_)))
        .expect("add record in fixtures")
}

fn add(uuid_byte: u8, name: &str) -> PlayerRecord {
    let mut r = template();
    let PlayerRecordContent::Add(a) = &mut r.content else { unreachable!() };
    a.uuid = Uuid([uuid_byte; 16]);
    a.username = name.into();
    a.xbox_user_id = format!("xuid-{name}");
    a.entity_unique_id = -(uuid_byte as i64);
    a.build_platform = 7;
    r
}

fn remove(uuid_byte: u8) -> PlayerRecord {
    PlayerRecord {
        r#type: PlayerRecordType::Remove,
        legacy_type: 1,
        content: PlayerRecordContent::Remove(PlayerRecordContentRemove { uuid: Uuid([uuid_byte; 16]) }),
    }
}

fn list(records: Vec<PlayerRecord>) -> RawPacket {
    raw(&PlayerListPacket { records })
}

#[test]
fn add_then_remove() {
    let mut pl = PlayerList::default();
    pl.apply(&list(vec![add(1, "Alice"), add(2, "Bob")])).unwrap();
    assert_eq!(pl.len(), 2);

    let alice = pl.get(&Uuid([1; 16])).unwrap();
    assert_eq!(alice.username, "Alice");
    assert_eq!(alice.xuid, "xuid-Alice");
    assert_eq!(alice.entity_unique_id, -1);
    assert_eq!(alice.build_platform, 7);
    assert_eq!(pl.by_name("bob").unwrap().uuid, Uuid([2; 16]));

    pl.apply(&list(vec![remove(1)])).unwrap();
    assert_eq!(pl.len(), 1);
    assert!(pl.get(&Uuid([1; 16])).is_none());
    assert!(pl.by_name("Alice").is_none());
    assert_eq!(pl.iter().map(|p| p.username.as_str()).collect::<Vec<_>>(), ["Bob"]);
}

#[test]
fn re_add_replaces_entry() {
    let mut pl = PlayerList::default();
    pl.apply(&list(vec![add(1, "Old")])).unwrap();
    pl.apply(&list(vec![add(1, "New")])).unwrap();
    assert_eq!(pl.len(), 1);
    assert_eq!(pl.get(&Uuid([1; 16])).unwrap().username, "New");
}

#[test]
fn records_joins_and_leaves_but_not_updates() {
    let mut pl = PlayerList::default();
    pl.apply(&list(vec![add(1, "Alice"), add(1, "Alice"), remove(2), remove(1)])).unwrap();
    let names: Vec<_> = pl
        .changes
        .iter()
        .map(|c| match c {
            PlayerChange::Joined(p) => format!("+{}", p.username),
            PlayerChange::Left(p) => format!("-{}", p.username),
        })
        .collect();
    assert_eq!(names, ["+Alice", "-Alice"]);
}
