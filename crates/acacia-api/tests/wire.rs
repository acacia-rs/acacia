//! What the JSON looks like on the wire (contract: the crate docs).

use acacia_api::*;
use serde_json::{json, Value};

#[test]
fn a_command_parses_into_its_method() {
    let cmd = Command::from_json(r#"{"call": 7, "method": "bot.goto", "params": {"bot": "bot-1", "goal": {"type": "near", "x": 1, "y": 64, "z": -3, "range": 2}}}"#).unwrap();
    assert_eq!(cmd.call, CallId(7));
    assert_eq!(cmd.method.lane(), Lane::Queued);
    assert_eq!(cmd.method.bot(), Some(&BotId::from("bot-1")));
    let Method::Goto(p) = &cmd.method else { panic!("{cmd:?}") };
    assert_eq!(p.goal, Goal::Near { x: 1, y: 64, z: -3, range: 2.0 });
    assert_eq!(Command::from_json(&cmd.to_json()).unwrap(), cmd, "round trip");
}

#[test]
fn bad_commands_say_what_is_wrong() {
    let (call, e) = Command::from_json(r#"{"call": 1, "method": "bot.fly", "params": {}}"#).unwrap_err();
    assert_eq!((call, e.code), (Some(CallId(1)), ErrorCode::UnknownMethod));
    let (call, e) = Command::from_json(r#"{"call": 2, "method": "bot.chat", "params": {"bot": "bot-1"}}"#).unwrap_err();
    assert_eq!((call, e.code), (Some(CallId(2)), ErrorCode::BadParams));
    assert!(e.message.contains("message"), "{e}");
    let (call, e) = Command::from_json(r#"{"method": "bot.chat"}"#).unwrap_err();
    assert_eq!((call, e.code), (None, ErrorCode::BadParams));
}

#[test]
fn records_are_tagged_camel_case_objects() {
    let records = [
        Out::Event { bot: "bot-1".into(), seq: 3, event: Event::WindowClose { window_id: 2 } },
        Out::Done { bot: "bot-1".into(), call: CallId(9), result: CallResult::ok(&Reply::Chat(())) },
        Out::Done { bot: "bot-1".into(), call: CallId(10), result: CallResult::Err(ApiError::new(ErrorCode::NotVisible, "behind glass")) },
    ];
    let json: Value = serde_json::from_str(&batch_json(&records)).unwrap();
    assert_eq!(
        json,
        json!([
            {"type": "event", "bot": "bot-1", "seq": 3, "event": {"type": "windowClose", "windowId": 2}},
            {"type": "done", "bot": "bot-1", "call": 9, "result": {"ok": null}},
            {"type": "done", "bot": "bot-1", "call": 10, "result": {"err": {"code": "NOT_VISIBLE", "message": "behind glass"}}},
        ])
    );
}

#[test]
fn every_method_has_a_doc_and_a_bot_except_host_ones() {
    for m in METHODS {
        assert!(!m.doc.trim().is_empty(), "{} has no doc", m.name);
    }
    let create = Method::parse("createBot", json!({"host": "127.0.0.1", "username": "Sprout"})).unwrap();
    assert_eq!((create.lane(), create.bot()), (Lane::Host, None));
}
