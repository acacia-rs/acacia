use super::*;

fn encode(value: &impl Wire) -> Vec<u8> {
    let mut out = Vec::new();
    value.write(&mut out);
    out
}

fn round_trip<T: Wire + PartialEq + std::fmt::Debug>(value: T) -> Vec<u8> {
    let out = encode(&value);
    let mut input = &out[..];
    assert_eq!(T::read(&mut input), Ok(value));
    assert!(input.is_empty());
    out
}

#[test]
fn plain_text_is_a_bare_string_tag() {
    // TAG_String, u16 length, bytes
    assert_eq!(round_trip(Component::text("hi")), [8, 0, 2, b'h', b'i']);
    assert_eq!(Component::from("x").to_tag(), Tag::String("x".into()));
}

#[test]
fn styled_text_is_a_compound() {
    let c = Component::text("a").color(NamedColor::Red).bold(true);
    assert_eq!(
        c.to_tag(),
        Tag::Compound(vec![
            ("text".into(), Tag::String("a".into())),
            ("color".into(), Tag::String("red".into())),
            ("bold".into(), Tag::Byte(1)),
        ])
    );
    round_trip(c);
}

#[test]
fn rgb_colours_serialise_as_hex() {
    let c = Component::text("a").color(Color::Rgb(0x12abef));
    assert_eq!(c.to_tag().get("color"), Some(&Tag::String("#12ABEF".into())));
    round_trip(c);
    assert_eq!(Color::parse("#12"), Err(Error::Invalid("text colour")));
    assert_eq!(Color::parse("pink"), Err(Error::Invalid("text colour")));
}

#[test]
fn every_flag_round_trips() {
    round_trip(Component::text("a").bold(true).italic(false).underlined(true).strikethrough(false).obfuscated(true));
}

#[test]
fn translatable_with_mixed_args_wraps_the_list() {
    let c = Component::translate("chat.type.text", vec![Component::text("Steve"), Component::text("hi").color(NamedColor::Gold)])
        .fallback("<%s> %s");
    let bytes = round_trip(c.clone());
    // `with` mixes a string and a compound, so it is a list of compounds with the string wrapped as {"": "Steve"}.
    let wrapped = [10, 0, 0, 0, 2, 8, 0, 0, 0, 5];
    assert!(bytes.windows(wrapped.len()).any(|w| w == wrapped), "{bytes:?}");
    assert_eq!(c.to_tag().get("fallback"), Some(&Tag::String("<%s> %s".into())));
}

#[test]
fn translatable_without_args_omits_with() {
    let c = Component::translate("block.minecraft.stone", Vec::new());
    assert_eq!(c.to_tag(), Tag::Compound(vec![("translate".into(), Tag::String("block.minecraft.stone".into()))]));
    round_trip(c);
}

#[test]
fn siblings_go_in_extra() {
    let c = Component::empty().append(Component::text("a").color(NamedColor::Green)).append(Component::text("b"));
    let tag = c.to_tag();
    assert!(matches!(tag.get("extra"), Some(Tag::List(items)) if items.len() == 2));
    round_trip(c);
}

#[test]
fn a_list_root_decodes_as_first_plus_siblings() {
    let tag = Tag::List(vec![Tag::String("a".into()), Tag::String("b".into())]);
    assert_eq!(Component::from_tag(&tag), Ok(Component::text("a").append(Component::text("b"))));
}

#[test]
fn hostile_components_are_rejected() {
    assert_eq!(Component::from_tag(&Tag::Compound(Vec::new())), Err(Error::Invalid("component content")));
    assert_eq!(Component::from_tag(&Tag::List(Vec::new())), Err(Error::Invalid("empty component list")));
    let mut nested = Tag::String("x".into());
    for _ in 0..100 {
        nested = Tag::List(vec![nested]);
    }
    assert_eq!(Component::from_tag(&nested), Err(Error::TooDeep(64)));
    assert_eq!(Component::read(&mut &[0][..]), Err(Error::Invalid("text component")));
}

#[test]
fn json_matches_the_vanilla_shapes() {
    assert_eq!(Component::text("a\"b\n").to_json(), r#""a\"b\n""#);
    let c = Component::translate("k", vec![Component::text("x"), Component::text("y").bold(true)]).color(NamedColor::Red);
    assert_eq!(c.to_json(), r#"{"translate":"k","with":["x",{"text":"y","bold":true}],"color":"red"}"#);
    let c = Component::text("a").color(Color::Rgb(0xff0000)).italic(false).append(Component::text("b"));
    assert_eq!(c.to_json(), r##"{"text":"a","color":"#FF0000","italic":false,"extra":["b"]}"##);
    assert_eq!(Component::text("\u{1}").to_json(), r#""\u0001""#);
}

#[test]
fn legacy_codes_become_runs() {
    assert_eq!(Component::from_legacy("plain"), Component::text("plain"));
    assert_eq!(Component::from_legacy(""), Component::empty());
    let c = Component::from_legacy("§cred §lbold§r done§zx");
    assert_eq!(
        c,
        Component::empty()
            .append(Component::text("red ").color(NamedColor::Red))
            .append(Component::text("bold").color(NamedColor::Red).bold(true))
            .append(Component::text(" donex"))
    );
    // a colour code clears earlier flags
    assert_eq!(Component::from_legacy("§l§aa"), Component::text("a").color(NamedColor::Green));
}

#[test]
fn number_formats_round_trip() {
    assert_eq!(round_trip(NumberFormat::Blank), [0]);
    assert_eq!(round_trip(NumberFormat::Fixed(Component::text("7"))), [2, 8, 0, 1, b'7']);
    // styled: id 1, then the style compound (type 10, entries, end)
    let styled = round_trip(NumberFormat::Styled(Style { color: Some(Color::Named(NamedColor::Red)), ..Style::default() }));
    assert_eq!(&styled[..2], &[1, 10]);
    assert_eq!(round_trip(NumberFormat::Styled(Style::default())), [1, 10, 0]);
    assert_eq!(round_trip(None::<NumberFormat>), [0]);
    assert_eq!(round_trip(Some(NumberFormat::Blank)), [1, 0]);
    assert_eq!(NumberFormat::read(&mut &[3][..]), Err(Error::Invalid("number format")));
}
