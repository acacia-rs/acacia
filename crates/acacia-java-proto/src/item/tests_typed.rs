use super::typed::*;
use super::*;
use crate::text::Component;
use crate::types::{AttributeModifierOperation, Filterable};

fn stack_with(components: Components) -> ItemStack {
    ItemStack { item: 812, count: 3, components }
}

fn round_trip(stack: &ItemStack) -> ItemStack {
    let mut out = Vec::new();
    Some(stack.clone()).write(&mut out);
    out.push(0xee);
    let mut input = &out[..];
    let read = <Option<ItemStack>>::read(&mut input).expect("every value passes the skipper");
    assert_eq!(input, [0xee], "no bytes left in the patch");
    read.unwrap()
}

fn one<C: ComponentValue + std::fmt::Debug + PartialEq>(value: C) {
    let stack = stack_with(Components::default().with(&value));
    let back = round_trip(&stack);
    assert_eq!(back, stack, "{value:?}");
    assert_eq!(back.components.get::<C>().unwrap(), Some(value));
}

fn effect(id: i32) -> MobEffectInstance {
    MobEffectInstance {
        effect: id,
        details: EffectDetails {
            amplifier: 1,
            duration: 600,
            ambient: false,
            visible: true,
            show_icon: true,
            hidden: Some(Box::new(EffectDetails { amplifier: 0, duration: 20, ..EffectDetails::default() })),
        },
    }
}

#[test]
fn every_typed_component_round_trips() {
    one(ItemName(Component::text("Blade")));
    one(Lore(vec![Component::text("a"), Component::translate("k", vec![]).italic(false)]));
    one(DyedColor(0x12_3456));
    one(GlintOverride(true));
    one(StoredEnchantments(vec![(3, 2), (40, 1)]));
    one(SuspiciousStewEffects(vec![(8, 160)]));
    one(Trim { material: 4, pattern: 11 });
    one(BannerPatterns(vec![BannerLayer { pattern: 7, color: 14 }, BannerLayer { pattern: 0, color: 0 }]));
    one(WritableBook(vec![Filterable { raw: "page".into(), filtered: Some("p***".into()) }, Filterable::default()]));
    one(WrittenBook {
        title: Filterable { raw: "Title".into(), filtered: None },
        author: "me".into(),
        generation: 1,
        pages: vec![Filterable { raw: Component::text("one"), filtered: None }],
        resolved: true,
    });
    one(PotionContents { potion: Some(5), custom_color: Some(0x00ff_00ff), effects: vec![effect(1), effect(2)], custom_name: Some("brew".into()) });
    one(PotionContents::default());
    one(Profile {
        uuid: 0x1234_5678_9abc_def0,
        name: "Steve".into(),
        properties: vec![ProfileProperty { name: "textures".into(), value: "v".into(), signature: Some("sig".into()) }],
    });
    one(AttributeModifiers(vec![
        AttributeEntry {
            attribute: 3,
            id: "minecraft:base_attack_damage".into(),
            amount: 7.5,
            operation: AttributeModifierOperation::AddMultipliedTotal,
            slot_group: 1,
            display: AttributeDisplay::Override(Component::text("+7.5")),
        },
        AttributeEntry {
            attribute: 4,
            id: "x:y".into(),
            amount: -1.0,
            operation: AttributeModifierOperation::AddValue,
            slot_group: 0,
            display: AttributeDisplay::Hidden,
        },
    ]));
}

#[test]
fn typed_and_field_components_mix_in_registry_order() {
    let mut components = Components { damage: Some(40), map_id: Some(9), enchantments: vec![(1, 5)], ..Components::default() };
    components.set_custom_name(&Component::text("Pick"));
    components.set(&DyedColor(7));
    components.set(&Lore(vec![Component::text("l")]));
    let stack = stack_with(components);
    let mut out = Vec::new();
    Some(stack.clone()).write(&mut out);
    let mut input = &out[..];
    read_varint(&mut input).unwrap();
    read_varint(&mut input).unwrap();
    assert_eq!(read_varint(&mut input).unwrap(), 6, "damage, name, enchantments, map id, lore, colour");
    assert_eq!(round_trip(&stack), stack);
    assert_eq!(stack.components.extra.iter().map(|(id, _)| *id).collect::<Vec<_>>(), [LORE, DYED_COLOR]);
}

#[test]
fn setting_twice_replaces_and_lore_is_capped() {
    let mut components = Components::default().with(&DyedColor(1));
    components.set(&DyedColor(2));
    assert_eq!(components.extra.len(), 1);
    assert_eq!(components.get::<DyedColor>().unwrap(), Some(DyedColor(2)));
    assert_eq!(components.get::<GlintOverride>().unwrap(), None);

    components.set(&Lore(vec![Component::text("x"); MAX_LORE_LINES + 10]));
    assert_eq!(components.get::<Lore>().unwrap().unwrap().0.len(), MAX_LORE_LINES);
}

#[test]
fn dyed_color_bytes_are_one_big_endian_int() {
    let mut out = Vec::new();
    DyedColor(0x00ab_cdef).write_value(&mut out);
    assert_eq!(out, [0, 0xab, 0xcd, 0xef]);
    let mut out = Vec::new();
    Trim { material: 0, pattern: 2 }.write_value(&mut out);
    assert_eq!(out, [1, 3], "holders write id + 1");
}

#[test]
fn inline_holders_and_partial_profiles_are_refused() {
    assert!(Trim::read_value(&mut &[0u8, 1][..]).is_err());
    assert!(Profile::read_value(&mut &[0u8][..]).is_err());
}

#[test]
fn typed_components_survive_in_merchant_style_exact_lists() {
    let components = Components::default().with(&PotionContents { potion: Some(3), ..PotionContents::default() });
    let mut out = Vec::new();
    components.write_exact(&mut out);
    assert_eq!(Components::read_exact(&mut &out[..]), Ok(components));
}
