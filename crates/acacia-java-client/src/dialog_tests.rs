use acacia_java_proto::dialog::{Action, Dialog};
use acacia_java_proto::tag::Tag;

use super::*;

fn s(v: &str) -> Tag {
    Tag::String(v.into())
}

fn c(entries: &[(&str, Tag)]) -> Tag {
    Tag::Compound(entries.iter().map(|(k, v)| ((*k).to_owned(), v.clone())).collect())
}

/// The research's multi_action with every input type (docs/java-edition.md "Forms").
fn inputs_dialog() -> Dialog {
    let input = |kind: &str, key: &str, rest: &[(&str, Tag)]| {
        let mut entries = vec![("type", s(kind)), ("key", s(key)), ("label", s(key))];
        entries.extend_from_slice(rest);
        c(&entries)
    };
    let tag = c(&[
        ("type", s("minecraft:multi_action")),
        ("title", s("Inputs")),
        (
            "inputs",
            Tag::List(vec![
                input("minecraft:text", "name", &[("initial", s("bob"))]),
                input("minecraft:boolean", "ok", &[("on_true", s("yes")), ("on_false", s("no"))]),
                input("minecraft:single_option", "col", &[("options", Tag::List(vec![s("red"), c(&[("id", s("blue")), ("display", s("Blue")), ("initial", Tag::Byte(1))])]))]),
                input("minecraft:number_range", "n", &[("start", Tag::Float(0.0)), ("end", Tag::Float(10.0)), ("step", Tag::Float(1.0)), ("initial", Tag::Float(5.0))]),
            ]),
        ),
        (
            "actions",
            Tag::List(vec![
                c(&[("label", s("Cmd")), ("action", c(&[("type", s("minecraft:dynamic/run_command")), ("template", s("tellraw @a \"$(name) $(ok) $(col) $(n)\""))]))]),
                c(&[("label", s("Custom")), ("action", c(&[("type", s("minecraft:dynamic/custom")), ("id", s("acacia:form")), ("additions", c(&[("v", Tag::Int(1))]))]))]),
            ]),
        ),
    ]);
    Dialog::from_tag(&tag).expect("parses")
}

#[test]
fn inputs_fill_templates_and_payloads_as_vanilla() {
    let dialog = inputs_dialog();
    let values = initial_values(&dialog.inputs);
    assert_eq!(values, vec![Value::Text("bob".into()), Value::Bool(false), Value::Option(1), Value::Number(5.0)]);
    let values = [Value::Text("bob".into()), Value::Bool(true), Value::Option(1), Value::Number(5.0)];
    assert_eq!(effect(&dialog.inputs, &dialog.buttons[0], &values), Effect::Command("tellraw @a \"bob yes blue 5\"".into()));
    let payload = c(&[("v", Tag::Int(1)), ("name", s("bob")), ("ok", Tag::Byte(1)), ("col", s("blue")), ("n", Tag::Float(5.0))]);
    assert_eq!(effect(&dialog.inputs, &dialog.buttons[1], &values), Effect::Custom { id: "acacia:form".into(), payload: Some(payload) });
}

#[test]
fn text_is_escaped_and_numbers_snap() {
    let dialog = inputs_dialog();
    let values = [Value::Text("a\"b".into()), Value::Bool(false), Value::Option(0), Value::Number(2.5)];
    assert_eq!(effect(&dialog.inputs, &dialog.buttons[0], &values), Effect::Command("tellraw @a \"a\\\"b no red 2.5\"".into()));
    assert_eq!(snap(6.6, 0.0, 10.0, 5.0, Some(1.0)), 7.0);
    assert_eq!(snap(10.4, 0.0, 10.0, 0.5, Some(1.0)), 9.5);
}

#[test]
fn confirmation_cancels_with_no() {
    let button = |label: &str, id: &str| c(&[("label", s(label)), ("action", c(&[("type", s("minecraft:custom")), ("id", s(id))]))]);
    let tag = c(&[("type", s("confirmation")), ("title", s("Sure?")), ("yes", button("Yes", "a:yes")), ("no", button("No", "a:no"))]);
    let dialog = Dialog::from_tag(&tag).expect("parses");
    assert_eq!(dialog.buttons.len(), 2);
    assert_eq!(dialog.cancel.as_ref().and_then(|b| b.action.clone()), Some(Action::Custom { id: "a:no".into(), payload: None }));
}
