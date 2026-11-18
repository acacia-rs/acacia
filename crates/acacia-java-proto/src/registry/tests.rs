use super::*;

#[test]
fn ids_match_the_report() {
    assert_eq!(MOB_EFFECT.id("minecraft:speed"), Some(0));
    assert_eq!(PARTICLE_TYPE.id("minecraft:angry_villager"), Some(0));
    assert_eq!(PARTICLE_TYPE.id("minecraft:dust"), Some(21));
    assert_eq!(MENU.id("minecraft:generic_9x3"), Some(2));
    assert_eq!(ATTRIBUTE.id("minecraft:max_health"), Some(23));
    assert_eq!(ATTRIBUTE.id("minecraft:movement_speed"), Some(26));
    assert_eq!(POTION.id("minecraft:awkward"), Some(3));
    assert_eq!(BLOCK_ENTITY_TYPE.id("minecraft:hanging_sign"), Some(8));
    assert_eq!(VILLAGER_TYPE.id("minecraft:plains"), Some(2));
    assert_eq!(SOUND_EVENT.id("minecraft:block.note_block.harp"), Some(1171));
    assert_eq!(NUMBER_FORMAT_TYPE.names().collect::<Vec<_>>(), ["minecraft:blank", "minecraft:styled", "minecraft:fixed"]);
}

#[test]
fn names_and_ids_are_inverse_and_unique() {
    for registry in ALL {
        assert!(!registry.is_empty(), "{}", registry.key);
        for (id, name) in registry.names().enumerate() {
            assert_eq!(registry.id(name), Some(id as i32), "{} {name}", registry.key);
            assert_eq!(registry.name(id as i32), Some(name));
        }
        assert_eq!(registry.name(registry.len() as i32), None);
        assert_eq!(registry.name(-1), None);
    }
    assert_eq!(SOUND_EVENT.len(), 1991);
}

#[test]
fn lenient_lookup_adds_the_namespace_once() {
    assert_eq!(MOB_EFFECT.id_lenient("speed"), Some(0));
    assert_eq!(MOB_EFFECT.id_lenient("other:speed"), None);
    assert_eq!(MOB_EFFECT.require("nope"), Err(Error::Invalid("registry entry")));
}

#[test]
fn entries_and_holders_write_varints() {
    let mut out = Vec::new();
    SOUND_EVENT.write_entry("block.note_block.harp", &mut out).unwrap();
    SOUND_EVENT.write_holder_ref("block.note_block.harp", &mut out).unwrap();
    let mut input = &out[..];
    assert_eq!(VarInt::read(&mut input), Ok(VarInt(1171)));
    assert_eq!(VarInt::read(&mut input), Ok(VarInt(1172)));
    assert!(input.is_empty());
}
