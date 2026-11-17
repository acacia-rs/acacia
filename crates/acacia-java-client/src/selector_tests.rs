use super::*;

fn parse(s: &str) -> (Result<Selector, SelectorError>, usize) {
    let units: Vec<u16> = s.encode_utf16().collect();
    let mut r = Reader::new(&units);
    let result = parse_selector(&mut r, true);
    (result, r.cursor)
}

fn extent(s: &str) -> Option<usize> {
    match parse(s) {
        (Ok(_), at) => Some(at),
        _ => None,
    }
}

#[test]
fn options_read_their_own_syntax() {
    assert_eq!(extent("@a[name=Bob, tag=x] hi"), Some(19));
    assert_eq!(extent("@e[ type = !minecraft:cow , nbt={a:[1,2]} ] hi"), Some(43));
    assert_eq!(extent("@p[distance=..5.5,x_rotation=-90..90] hi"), Some(37));
    assert_eq!(extent("@a[scores={a=1..2, b=3},advancements={story/root=true,x={y=false}}] hi"), Some(67));
    assert_eq!(extent("@e[type=#minecraft:skeletons,type=!pig]"), Some(39));
    assert_eq!(extent("Steve rest"), Some(5));
    assert_eq!(extent("\"Steve\" rest"), Some(7));
    assert_eq!(extent("0-0-0-0-0 rest"), Some(9));
}

#[test]
fn bad_selectors_fail_as_vanilla_does() {
    assert_eq!(parse("@").0, Err(SelectorError::MissingType));
    assert_eq!(parse("@x").0, Err(SelectorError::UnknownType));
    for bad in [
        "@e[type=cow,type=pig]",
        "@s[limit=1]",
        "@a[limit=0]",
        "@a[distance=-1]",
        "@a[level=5..3]",
        "@a[sort=closest]",
        "@a[name=a,name=b]",
        "@a[colour=red]",
        "@e[type=not_a_mob]",
        "@a[tag=x",
        "AVeryLongPlayerName",
    ] {
        assert_eq!(extent(bad), None, "{bad}");
    }
}

#[test]
fn results_carry_the_limit_and_player_flags() {
    let flags = |s| parse(s).0.map(|s| (s.max_results, s.includes_entities, s.current_entity));
    assert_eq!(flags("@e[limit=1]"), Ok((1, true, false)));
    assert_eq!(flags("@e[type=player]"), Ok((i32::MAX, false, false)));
    assert_eq!(flags("@s"), Ok((1, true, true)));
    assert_eq!(flags("Bob"), Ok((1, false, false)));
}
