use acacia_registry::{Block, Item, ItemTag};

#[test]
fn java_ids_and_components() {
    assert_eq!(Item::Air.java_id(), 0);
    assert_eq!(Item::from_java_id(Item::Stone.java_id()), Some(Item::Stone));
    assert_eq!(Item::EnderPearl.max_stack(), 16);
    assert_eq!(Item::DiamondSword.max_stack(), 1);
    assert_eq!(Item::Stone.max_stack(), 64);
    assert_eq!(Item::OakLog.block(), Some(Block::OakLog));
    assert_eq!(Item::DiamondSword.block(), None);
}

#[test]
fn bedrock_names_and_data() {
    assert_eq!(Item::from_bedrock("minecraft:web", 0), Some(Item::Cobweb));
    assert_eq!(Item::Cobweb.bedrock(), ("minecraft:web", 0));
    assert_eq!(Item::from_bedrock("minecraft:bed", 14), Some(Item::RedBed));
    assert_eq!(Item::from_bedrock("minecraft:bed", 0), Some(Item::WhiteBed));
    assert_eq!(Item::RedBed.bedrock(), ("minecraft:bed", 14));
    assert_eq!(Item::from_bedrock("minecraft:banner", 1), Some(Item::RedBanner));
    // Pre-flattening data on a flattened item is ignored.
    assert_eq!(Item::from_bedrock("minecraft:acacia_planks", 4), Some(Item::AcaciaPlanks));
    assert_eq!(Item::AcaciaPlanks.bedrock(), ("minecraft:acacia_planks", 0));
    assert_eq!(Item::from_bedrock("minecraft:filled_map", 0), Some(Item::FilledMap));
    assert_eq!(Item::from_bedrock("minecraft:no_such_item", 0), None);
}

#[test]
fn every_item_round_trips_through_bedrock() {
    for item in Item::all() {
        let (name, data) = item.bedrock();
        let back = Item::from_bedrock(name, data).unwrap_or_else(|| panic!("{item}"));
        // Merged names land on their main item (maps → filled_map, debug_stick → stick).
        assert_eq!(back.bedrock(), (name, data), "{item} -> {back}");
    }
}

#[test]
fn names_and_tags() {
    assert_eq!("web".parse::<Item>(), Ok(Item::Cobweb));
    assert_eq!("minecraft:cobweb".parse::<Item>(), Ok(Item::Cobweb));
    assert!("minecraft:bed".parse::<Item>().is_err());
    assert!(Item::OakLog.is(ItemTag::Logs));
    assert!(Item::DiamondPickaxe.is(ItemTag::Pickaxes));
    assert!(!Item::Stick.is(ItemTag::Logs));
}
