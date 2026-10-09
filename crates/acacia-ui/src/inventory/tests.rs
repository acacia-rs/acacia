use super::*;

#[test]
fn slots_are_where_java_puts_them() {
    let p = Layout::Player;
    assert_eq!(slots(p).len(), 4 + 36 + 4 + 2);
    let size = [320.0, 240.0];
    let [ox, oy] = origin(p, size);
    assert_eq!([ox, oy], [72.0, 37.0]);
    assert_eq!(hit(p, size, [ox + 8.0 + 18.0 * 2.0 + 3.0, oy + 142.0 + 5.0]), Some(Slot::Main(2)));
    assert_eq!(hit(p, size, [ox + 8.0 + 1.0, oy + 84.0 + 1.0]), Some(Slot::Main(9)));
    assert_eq!(hit(p, size, [ox + 8.0 + 18.0 * 8.0 + 1.0, oy + 84.0 + 36.0 + 1.0]), Some(Slot::Main(35)));
    assert_eq!(hit(p, size, [ox + 9.0, oy + 9.0 + 18.0 * 3.0]), Some(Slot::Armor(3)));
    assert_eq!(hit(p, size, [ox + 7.0, oy + 142.0]), None, "the gap between slots");
    assert!(!inside(p, size, [10.0, 10.0]));
}

#[test]
fn stations_put_their_slots_where_javas_menus_do() {
    let at = |layout, s| slots(layout).into_iter().find(|(slot, _)| *slot == s).unwrap().1;
    let furnace = Layout::Station(Station::Furnace);
    assert_eq!([at(furnace, Slot::Container(0)), at(furnace, Slot::Container(1)), at(furnace, Slot::Container(2))], [[56.0, 17.0], [56.0, 53.0], [116.0, 35.0]]);
    assert_eq!(at(furnace, Slot::Main(9)), [8.0, 84.0]);
    let hopper = Layout::Station(Station::Hopper);
    assert_eq!(hopper.height(), 133.0);
    assert_eq!(at(hopper, Slot::Container(4)), [116.0, 20.0]);
    assert_eq!(at(hopper, Slot::Main(0)), [8.0, 109.0]);
    assert_eq!(at(Layout::Station(Station::Dropper), Slot::Container(8)), [98.0, 53.0]);
    // Bedrock's order: ingredient, bottles, blaze powder.
    let brewing = Layout::Station(Station::Brewing);
    assert_eq!([at(brewing, Slot::Container(0)), at(brewing, Slot::Container(4))], [[79.0, 17.0], [17.0, 17.0]]);
}

#[test]
fn a_crafting_table_has_its_grid_and_result() {
    let table = Layout::Bench(Bench::Crafting);
    let at = |s| slots(table).into_iter().find(|(slot, _)| *slot == s).unwrap().1;
    assert_eq!([at(Slot::Ui(32)), at(Slot::Ui(40)), at(Slot::Result)], [[30.0, 17.0], [66.0, 53.0], [124.0, 35.0]]);
    assert_eq!(at(Slot::Main(9)), [8.0, 84.0]);
    assert!(table.has_book() && !Layout::Rows(3).has_book() && !Layout::Bench(Bench::Anvil).has_book());
    let smithing: Vec<Slot> = slots(Layout::Bench(Bench::Smithing)).into_iter().map(|(s, _)| s).skip(36).collect();
    assert_eq!(smithing, [Slot::Ui(53), Slot::Ui(51), Slot::Ui(52), Slot::Result], "template, base, addition");
    assert_eq!(slots(Layout::Player).into_iter().filter(|(s, _)| matches!(s, Slot::Ui(28..=31))).count(), 4);
}

#[test]
fn a_stonecutter_lists_its_cuts_four_to_a_row() {
    let (cutter, size) = (Layout::Bench(Bench::Stonecutter), [320.0, 240.0]);
    let [ox, oy] = origin(cutter, size);
    assert_eq!(hit_pick(cutter, size, 6, [ox + 52.0 + 16.0 + 1.0, oy + 14.0 + 18.0 + 1.0]), Some(5));
    assert_eq!(hit_pick(cutter, size, 5, [ox + 52.0 + 16.0 + 1.0, oy + 14.0 + 18.0 + 1.0]), None, "no sixth cut");
    assert_eq!(pick_rects(cutter, size, 40).len(), 12, "three rows show");
    assert_eq!(hit_pick(Layout::Player, size, 6, [ox + 53.0, oy + 15.0]), None);
    let table = Layout::Bench(Bench::Enchanting);
    assert_eq!(hit_pick(table, size, 3, [ox + 61.0, oy + 14.0 + 19.0 * 2.0 + 1.0]), Some(2), "one option to a row");
    assert!(!slots(table).iter().any(|(s, _)| *s == Slot::Result), "the item is enchanted in its slot");
}

#[test]
fn a_trade_screen_is_wider_with_the_offers_on_the_left() {
    let (trade, size) = (Layout::Trade, [427.0, 240.0]);
    let [ox, oy] = origin(trade, size);
    assert_eq!([ox, oy], [75.0, 37.0]);
    let at = |s| slots(trade).into_iter().find(|(slot, _)| *slot == s).unwrap().1;
    assert_eq!([at(Slot::Ui(4)), at(Slot::Ui(5)), at(Slot::Main(9)), at(Slot::Main(0))], [[136.0, 37.0], [162.0, 37.0], [108.0, 84.0], [108.0, 142.0]]);
    assert_eq!(hit_pick(trade, size, 9, [ox + 6.0, oy + 18.0 + 20.0 * 6.0 + 1.0]), Some(6));
    assert_eq!(pick_rects(trade, size, 9).len(), 7, "seven offers show");
    assert!(inside(trade, size, [ox + 270.0, oy + 10.0]) && !inside(Layout::Player, size, [ox + 270.0 + 50.0, oy + 10.0]));
}

#[test]
fn a_chest_sits_above_the_player_rows() {
    let chest = Layout::Rows(3);
    assert_eq!(chest.height(), 168.0);
    let all = slots(chest);
    assert_eq!(all.len(), 27 + 36);
    let at = |s| all.iter().find(|(slot, _)| *slot == s).unwrap().1;
    assert_eq!(at(Slot::Container(0)), [8.0, 18.0]);
    assert_eq!(at(Slot::Container(26)), [8.0 + 18.0 * 8.0, 18.0 + 36.0]);
    assert_eq!(at(Slot::Main(9)), [8.0, 85.0]);
    assert_eq!(at(Slot::Main(0)), [8.0, 143.0]);
    assert_eq!(Layout::Rows(6).height(), 222.0);
}
