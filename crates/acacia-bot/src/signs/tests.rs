use acacia_client::proto::nbt::{Nbt, Value};
use acacia_client::proto::packets::{BlockEntityData, OpenSign};
use acacia_client::proto::types::BlockCoordinates;

use super::{sign_nbt, SideEdit};
use crate::state::queries::test_support::raw;
use crate::state::{BlockEntities, BlockEntityTracking, SignEditor, Signs};

fn keys(v: &Value) -> Vec<&str> {
    match v {
        Value::Compound(entries) => entries.iter().map(|(k, _)| k.as_str()).collect(),
        _ => panic!("not a compound: {v:?}"),
    }
}

fn string<'a>(v: &'a Value, side: &str, key: &str) -> &'a str {
    match v.get(side).and_then(|s| s.get(key)) {
        Some(Value::String(s)) => s,
        other => panic!("{side}.{key}: {other:?}"),
    }
}

#[test]
fn fresh_sign_shaped_like_bds_in_key_order() {
    let edit = SideEdit { front: true, text: "hello\nworld" };
    let nbt = sign_nbt(None, [10, 64, -3], false, &edit, -42);
    let root = &nbt.value;
    assert_eq!(keys(root), ["BackText", "BlockEntityVersion", "FrontText", "IsWaxed", "LockedForEditingBy", "id", "x", "y", "z"]);
    assert_eq!(
        keys(root.get("FrontText").unwrap()),
        ["FilteredText", "HideGlowOutline", "IgnoreLighting", "PersistFormatting", "SignTextColor", "Text", "TextOwner"]
    );
    assert_eq!(string(root, "FrontText", "Text"), "hello\nworld");
    assert_eq!(string(root, "FrontText", "TextOwner"), "", "vanilla leaves the owner to the server");
    assert_eq!(string(root, "BackText", "Text"), "");
    assert_eq!(root.get("id"), Some(&Value::String("Sign".into())));
    assert_eq!(root.get("LockedForEditingBy"), Some(&Value::Long(-42)));
    assert_eq!((root.get("x"), root.get("z")), (Some(&Value::Int(10)), Some(&Value::Int(-3))));
    assert_eq!(sign_nbt(None, [0, 0, 0], true, &edit, 0).value.get("id"), Some(&Value::String("HangingSign".into())));

    // What goes on the wire decodes back unchanged.
    let packet = BlockEntityData { position: BlockCoordinates { x: 10, y: 64, z: -3 }, nbt: nbt.into() };
    assert_eq!(raw(&packet).decode::<BlockEntityData>().unwrap(), packet);
}

#[test]
fn edits_keep_the_servers_sign_and_sort_its_keys() {
    let side = |text: &str, colour: i32| {
        Value::Compound(vec![
            ("Text".into(), Value::String(text.into())),
            ("SignTextColor".into(), Value::Int(colour)),
            ("TextOwner".into(), Value::String("owner".into())),
            ("IgnoreLighting".into(), Value::Byte(1)),
        ])
    };
    let server = Nbt {
        name: String::new(),
        value: Value::Compound(vec![
            ("id".into(), Value::String("Sign".into())),
            ("FrontText".into(), side("front text", -65536)),
            ("BackText".into(), side("old back", -16777216)),
            ("CustomTag".into(), Value::Byte(3)),
        ]),
    };
    let edit = SideEdit { front: false, text: "new back" };
    let nbt = sign_nbt(Some(&server), [0, 70, 0], false, &edit, 7);
    let root = &nbt.value;
    assert_eq!(keys(root), ["BackText", "CustomTag", "FrontText", "id"]);
    assert_eq!(keys(root.get("BackText").unwrap()), ["IgnoreLighting", "SignTextColor", "Text", "TextOwner"]);
    assert_eq!(string(root, "BackText", "Text"), "new back");
    assert_eq!(string(root, "BackText", "TextOwner"), "owner", "only Text changes");
    assert_eq!(string(root, "FrontText", "Text"), "front text");
    assert_eq!(root.get("FrontText").unwrap().get("SignTextColor"), Some(&Value::Int(-65536)));
}

#[test]
fn trackers_keep_signs_and_the_open_editor() {
    let mut signs = Signs::default();
    signs.apply(&raw(&OpenSign { position: BlockCoordinates { x: 1, y: 2, z: 3 }, is_front: false })).unwrap();
    assert_eq!(signs.editor, Some(SignEditor { position: [1, 2, 3], front: false }));

    let mut entities = BlockEntities::new(BlockEntityTracking::Off);
    let edit = SideEdit { front: true, text: "shop" };
    let nbt = sign_nbt(None, [1, 2, 3], false, &edit, 0);
    entities.apply(&raw(&BlockEntityData { position: BlockCoordinates { x: 1, y: 2, z: 3 }, nbt: nbt.into() })).unwrap();
    assert_eq!(entities.sign_text([1, 2, 3], true), Some("shop"));
    assert_eq!(entities.sign_text([1, 2, 3], false), Some(""));

    let chest = Nbt { name: String::new(), value: Value::Compound(vec![("id".into(), Value::String("Chest".into()))]) };
    entities.apply(&raw(&BlockEntityData { position: BlockCoordinates { x: 5, y: 5, z: 5 }, nbt: chest.into() })).unwrap();
    assert!(entities.get([5, 5, 5]).is_none(), "only signs without chunk tracking");
}
