//! The socket protocol end to end against FakeServer (contract: the crate docs).

mod common;

use std::time::{Duration, Instant};

use acacia_testserver::{FakeServer, Script};
use common::{chat, daemon, error_code, handshake, server_got_chat, Client};
use serde_json::json;
use tokio::time::sleep;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn hello_checks_the_version_and_the_token() {
    let d = daemon(Some("s3cret")).await;
    let mut early = Client::connect(d.local_addr()).await;
    assert_eq!(early.err("bot.chat", json!({ "bot": "bot-1", "message": "hi" })).await, "UNAUTHORIZED");
    assert_eq!(early.err("hello", json!({ "protocolVersion": 99, "token": "s3cret" })).await, "VERSION_MISMATCH");
    assert_eq!(early.err("hello", json!({ "protocolVersion": 1, "token": "guess" })).await, "UNAUTHORIZED");
    early.wait_closed().await;

    let mut client = Client::connect(d.local_addr()).await;
    let info = client.ok("hello", json!({ "protocolVersion": acacia_api::PROTOCOL_VERSION, "token": "s3cret" })).await;
    assert_eq!(info["protocolVersion"], acacia_api::PROTOCOL_VERSION);
    assert_eq!((info["reads"].as_bool(), info["mirror"].as_bool()), (Some(false), Some(false)));
    assert_eq!(info["methods"].as_str().map(str::len), Some(16), "{info}");
    assert_eq!(client.ok("bots", json!({})).await, json!({ "bots": [] }));

    let open = daemon(None).await;
    assert!(handshake(open.local_addr(), Some("https://example.com")).await.is_err(), "browsers need a token");
    assert!(handshake(d.local_addr(), Some("https://example.com")).await.is_ok(), "a token admits them");
    d.shutdown().await;
    open.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn malformed_requests_get_json_rpc_errors() {
    let d = daemon(None).await;
    let mut c = Client::greeted(d.local_addr(), json!({})).await;
    let mut frame = async |text: &str| {
        c.send_text(text).await;
        let i = c.until("an error", |f| f.get("error").is_some()).await;
        c.seen.remove(i)
    };
    let parse = frame("{").await;
    assert_eq!((parse["error"]["code"].as_i64(), &parse["id"]), (Some(-32700), &json!(null)));
    assert_eq!(frame("[]").await["error"]["code"], -32600);
    let no_method = frame(r#"{"jsonrpc":"2.0","id":5}"#).await;
    assert_eq!((no_method["error"]["code"].as_i64(), &no_method["id"]), (Some(-32600), &json!(5)));
    let unknown = frame(r#"{"jsonrpc":"2.0","id":"u","method":"bot.fly","params":{}}"#).await;
    assert_eq!((unknown["error"]["code"].as_i64(), error_code(&unknown).as_str()), (Some(-32601), "UNKNOWN_METHOD"));
    let params = frame(r#"{"jsonrpc":"2.0","id":7,"method":"bot.chat","params":{}}"#).await;
    assert_eq!((params["error"]["code"].as_i64(), error_code(&params).as_str()), (Some(-32602), "BAD_PARAMS"));
    let foreign = frame(r#"{"jsonrpc":"2.0","id":8,"method":"bot.chat","params":{"bot":"bot-1","message":"x"}}"#).await;
    assert_eq!((foreign["error"]["code"].as_i64(), error_code(&foreign).as_str()), (Some(-32021), "UNKNOWN_BOT"));
    assert_eq!(c.err("attach", json!({})).await, "BAD_PARAMS");
    d.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn calls_round_trip_with_events_before_their_response() {
    let server = FakeServer::start(Script::bds_spawn()).await.unwrap();
    let d = daemon(None).await;
    let mut c = Client::greeted(d.local_addr(), json!({})).await;
    let bot = c.create(&server, "WsTester", json!({})).await;
    c.until("the mirrored hot snapshot", |r| r["type"] == "mirror" && r["bot"] == bot && r["hot"]["alive"] == true).await;

    assert_eq!(c.ok("bot.chat", json!({ "bot": bot, "message": "hello" })).await, json!(null));
    server_got_chat(&server, "hello").await;

    let wait = c.request("bot.waitForTicks", json!({ "bot": bot, "ticks": 30 })).await;
    sleep(Duration::from_millis(400)).await;
    chat(&server, "Steve", "mid-action");
    let (done_at, done) = c.response(wait).await;
    assert_eq!(done["result"], json!(null), "{done}");
    let chat_at = c.event_index(&bot, |e| e["type"] == "chat" && e["plain"] == "mid-action").expect("the chat event");
    assert!(chat_at < done_at, "the chat raised during the wait comes before its response");

    let long = c.request("bot.waitForTicks", json!({ "bot": bot, "ticks": 400 })).await;
    let started = Instant::now();
    c.ok("cancel", json!({ "bot": bot, "call": long })).await;
    assert_eq!(error_code(&c.response(long).await.1), "CANCELLED");
    assert!(started.elapsed() < Duration::from_secs(5), "cancel names the request id");

    c.ok("bot.close", json!({ "bot": bot })).await;
    let end = c.event_index(&bot, |e| e["type"] == "end").expect("end before close resolves");
    assert_eq!(c.seen[end]["event"]["reason"], "closed");
    d.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_dropped_connection_closes_its_bots() {
    let server = FakeServer::start(Script::bds_spawn()).await.unwrap();
    let d = daemon(None).await;
    let mut owner = Client::greeted(d.local_addr(), json!({})).await;
    let bot = owner.create(&server, "Owned", json!({})).await;
    let mut other = Client::greeted(d.local_addr(), json!({})).await;
    let listed = other.ok("bots", json!({})).await;
    assert_eq!(listed["bots"][0], json!({ "bot": bot, "username": "Owned", "persist": false, "attached": 1, "mine": false }));
    assert_eq!(other.err("attach", json!({ "username": "Owned" })).await, "UNKNOWN_BOT", "only persisted bots attach");
    assert_eq!(other.err("bot.chat", json!({ "bot": bot, "message": "x" })).await, "UNKNOWN_BOT");

    owner.close().await;
    let deadline = Instant::now() + Duration::from_secs(10);
    while other.ok("bots", json!({})).await["bots"] != json!([]) {
        assert!(Instant::now() < deadline, "the owner's bot outlived its connection");
        sleep(Duration::from_millis(50)).await;
    }
    d.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_persisted_bot_survives_its_connection_and_reattaches() {
    let server = FakeServer::start(Script::bds_spawn()).await.unwrap();
    let d = daemon(None).await;
    let mut first = Client::greeted(d.local_addr(), json!({})).await;
    let bot = first.create(&server, "Keeper", json!({ "persist": true })).await;
    first.request("bot.waitForTicks", json!({ "bot": bot, "ticks": 2000 })).await;
    sleep(Duration::from_millis(200)).await;
    first.close().await;

    sleep(Duration::from_millis(300)).await;
    let mut second = Client::greeted(d.local_addr(), json!({})).await;
    let listed = second.ok("bots", json!({})).await;
    assert_eq!(listed["bots"][0], json!({ "bot": bot, "username": "Keeper", "persist": true, "attached": 0, "mine": false }));
    let attached = second.ok("attach", json!({ "username": "Keeper" })).await;
    assert_eq!((&attached["bot"], &attached["persist"]), (&json!(bot), &json!(true)));
    assert_eq!(attached["snapshot"]["hot"]["alive"], true, "{attached}");

    let started = Instant::now();
    second.ok("bot.waitForTicks", json!({ "bot": bot, "ticks": 1 })).await;
    assert!(started.elapsed() < Duration::from_secs(5), "the dropped connection's wait was cancelled");
    chat(&server, "Steve", "welcome back");
    second.until("the chat", |r| r["type"] == "event" && r["bot"] == bot && r["event"]["plain"] == "welcome back").await;

    second.ok("bot.close", json!({ "bot": bot })).await;
    assert_eq!(second.ok("bots", json!({})).await, json!({ "bots": [] }));
    d.shutdown().await;
}
