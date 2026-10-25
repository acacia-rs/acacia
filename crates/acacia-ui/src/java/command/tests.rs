use serde_json::json;

use super::error::Kind;
use super::*;
use crate::java::chat::suggestions::Completion;
use crate::java::chat::{ChatScreen, Key};
use crate::java::gui::Fixed;

fn tree() -> Tree {
    Tree::from_json(&json!([
        {"literal": "give", "children": [{"argument": "player", "type": "token", "suggest": ["Steve", "Alex"], "children": [
            {"argument": "item", "type": "token", "exec": true, "children": [{"argument": "count", "type": "integer", "min": 1, "max": 64, "exec": true}]}]}]},
        {"literal": "time", "children": [
            {"literal": "set", "children": [{"argument": "value", "type": "integer", "exec": true}]},
            {"literal": "add", "children": [{"argument": "value", "type": "integer", "exec": true}]},
            {"literal": "query", "children": [{"literal": "daytime", "exec": true}, {"literal": "gametime", "exec": true}]}]},
        {"literal": "say", "children": [{"argument": "message", "type": "greedy", "exec": true}]}
    ]))
}

fn units(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

#[test]
fn arguments_parse_into_ranges() {
    let t = tree();
    let p = t.parse(&units("/give Steve stone 5"), 1);
    let args: Vec<(&str, usize, usize)> = p.context.arguments.iter().map(|(n, r)| (n.as_str(), r.start, r.end)).collect();
    assert_eq!(args, [("player", 6, 11), ("item", 12, 17), ("count", 18, 19)]);
    assert!(p.context.executable && !p.can_read() && p.errors.is_empty());
    assert_eq!(t.parse(&units("/say hi  there"), 1).context.arguments[0].1, Range { start: 5, end: 14 });
}

#[test]
fn a_bad_argument_leaves_its_error_and_the_rest_unread() {
    let p = tree().parse(&units("/give Steve stone x"), 1);
    assert_eq!(p.cursor, 18);
    let e = p.parse_error().expect("trailing input");
    assert_eq!((e.kind.clone(), e.cursor), (Kind::ExpectedInt, 18));
    assert_eq!(e.context().as_deref(), Some("...eve stone <--[HERE]"));
    let too_many = tree().parse(&units("/give Steve stone 65"), 1);
    assert_eq!(too_many.errors[0].1.kind, Kind::IntegerTooHigh { found: 65, max: 64 });
}

#[test]
fn completions_cover_the_word_at_the_cursor() {
    let t = tree();
    let input = units("/ti");
    let s = t.completions(&t.parse(&input, 1), 3);
    assert_eq!((s.range, s.list.iter().map(|x| x.text.as_str()).collect::<Vec<_>>()), (Some(Range { start: 1, end: 3 }), vec!["time"]));
    let input = units("/time ");
    let s = t.completions(&t.parse(&input, 1), 6);
    assert_eq!(s.list.iter().map(|x| x.text.as_str()).collect::<Vec<_>>(), ["add", "query", "set"]);
    let input = units("/give st");
    let s = t.completions(&t.parse(&input, 1), 8);
    assert_eq!(s.list.iter().map(|x| x.text.as_str()).collect::<Vec<_>>(), ["Steve"]);
}

#[test]
fn usage_marks_optional_and_alternative_parts() {
    let t = tree();
    let give = t.node(0).children[0];
    assert_eq!(t.smart_usage(give), [(t.node(give).children[0], "<player> <item> [<count>]".to_owned())]);
    let time = t.node(0).children[1];
    let usage: Vec<String> = t.smart_usage(time).into_iter().map(|(_, u)| u).collect();
    assert_eq!(usage, ["set <value>", "add <value>", "query (daytime|gametime)"]);
}

#[test]
fn the_chat_screen_colours_arguments_and_shows_usage() {
    let m = Fixed(6.0);
    let completion = Completion { commands: std::sync::Arc::new(tree()), auto_suggest: true, ..Completion::default() };
    let mut screen = ChatScreen::new(&m, "", 0, completion, 320, 240);
    "/give Steve ".chars().for_each(|c| screen.char_typed(&m, c));
    assert_eq!(screen.suggestions.usage.iter().map(|t| t.plain()).collect::<Vec<_>>(), ["<item> [<count>]"]);
    let colored = screen.suggestions.format("/give Steve ", 0).expect("a command");
    let spans: Vec<(&str, Option<u32>)> = colored.spans.iter().map(|s| (s.text.as_str(), s.style.color)).collect();
    // A lone trailing space is not read past (canRead(2)), so it is left over: red.
    assert_eq!(spans, [("/give ", Some(0xAAAAAA)), ("Steve", Some(0x55FFFF)), (" ", Some(0xFF5555))]);
    // An argument without suggestions shows no list; one with them shows it as soon as typed.
    screen.char_typed(&m, 's');
    assert!(screen.suggestions.list.is_none());
    let mut screen = ChatScreen::new(&m, "", 0, screen.completion.clone(), 320, 240);
    "/give A".chars().for_each(|c| screen.char_typed(&m, c));
    assert_eq!(screen.suggestions.list.as_ref().map(|l| l.list[0].text.as_str()), Some("Alex"));
    assert_eq!(screen.input.suggestion.as_deref(), Some("lex"));
    screen.key(&m, &[], Key::Tab, false, false);
    assert_eq!(screen.input.value(), "/give Alex");
}
