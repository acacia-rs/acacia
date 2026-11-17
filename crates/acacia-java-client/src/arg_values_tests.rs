use super::*;

fn extent(s: &str, f: impl Fn(&mut Reader) -> Parsed) -> Option<usize> {
    let units: Vec<u16> = s.encode_utf16().collect();
    let mut r = Reader::new(&units);
    f(&mut r).ok().map(|()| r.cursor)
}

#[test]
fn coordinates_take_one_space_between_parts() {
    assert_eq!(extent("~ ~1 -3 rest", |r| position(r, true)), Some(7));
    assert_eq!(extent("^ ^ ^2.5 rest", |r| position(r, false)), Some(8));
    assert_eq!(extent("1.5 2 3", |r| position(r, true)), None, "block positions are whole");
    assert_eq!(extent("~ ^ ~", |r| position(r, false)), None, "mixed");
    assert_eq!(extent("1  2 3", |r| position(r, false)), Some(4), "an empty part reads as 0");
    assert_eq!(extent("~10 ~ x", |r| pair(r, false)), Some(5));
}

#[test]
fn blocks_and_items_read_their_brackets() {
    assert_eq!(extent("minecraft:oak_stairs[facing=east, half=\"top\"]{a:1} 5", |r| block_state(r, false)), Some(50));
    assert_eq!(extent("#minecraft:logs[axis=y] x", |r| block_state(r, true)), Some(23));
    assert_eq!(extent("#minecraft:logs", |r| block_state(r, false)), None);
    assert_eq!(extent("stone[a=1,a=2]", |r| block_state(r, false)), None);
    assert_eq!(extent("diamond_sword[damage=5, !enchantments] 1", |r| item_stack(r)), Some(38));
    assert_eq!(extent("stick[custom_name=\"a b\"] 1", |r| item_stack(r)), Some(24));
    assert_eq!(extent("*[damage~{durability:{min:1}}|!custom_data] 2", |r| item_predicate(r)), Some(43));
    assert_eq!(extent("#minecraft:logs 3", |r| item_predicate(r)), Some(15));
    assert_eq!(extent("stone [damage=1] 3", |r| item_predicate(r)), Some(16), "terminals skip whitespace");
}

#[test]
fn nbt_paths_end_at_a_space() {
    assert_eq!(extent("a.b[0].\"c d\"[{x:1}].e{f:2} rest", nbt_path), Some(26));
    assert_eq!(extent("{a:1}.b rest", nbt_path), Some(7));
    assert_eq!(extent("a.{b:1}", nbt_path), None);
    assert_eq!(extent("a[", nbt_path), None);
}

#[test]
fn messages_take_the_rest_and_check_selectors_when_allowed() {
    assert_eq!(extent("hi @a there", |r| message(r, true)), Some(11));
    assert_eq!(extent("mail me @ example.com, @x", |r| message(r, true)), Some(25), "missing and unknown types are text");
    assert_eq!(extent("hi @a[limit=0]", |r| message(r, true)), None);
    assert_eq!(extent("hi @a[limit=0]", |r| message(r, false)), Some(14));
    assert_eq!(extent(&"x".repeat(257), |r| message(r, false)), None);
}
