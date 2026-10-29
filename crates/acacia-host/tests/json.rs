//! Driving bots purely through command JSON and reading the record stream back (contract: the crate docs).

use std::time::{Duration, Instant};

use acacia_api::{BlockPos, BotId, ErrorCode};
use acacia_bot::proto::packets::{Text, TextCategory, TextContent, TextContentRaw, TextType};
use acacia_host::{Host, HostConfig, Outbound};
use acacia_testserver::{FakeServer, Script};
use serde_json::{json, Value};
use tokio::time::{sleep, timeout};

struct Driver {
    host: Host,
    out: Outbound,
    /// Every record read so far, in stream order.
    seen: Vec<Value>,
}

impl Driver {
    fn start() -> Self {
        let (host, out) = Host::start(HostConfig { shards: 2, ..HostConfig::default() }).unwrap();
        Self { host, out, seen: Vec::new() }
    }

    fn call(&self, call: u64, method: &str, params: Value) {
        let command = json!({ "call": call, "method": method, "params": params }).to_string();
        self.host.call(&command).unwrap_or_else(|e| panic!("{method} refused: {e}"));
    }

    /// Reads the stream until a record matches `want`; returns its index in `seen`.
    async fn until(&mut self, what: &str, want: impl Fn(&Value) -> bool) -> usize {
        if let Some(i) = self.seen.iter().position(&want) {
            return i;
        }
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            let batch = timeout(left, self.out.recv()).await.unwrap_or_else(|_| panic!("no {what}; seen {:#?}", self.seen));
            let batch: Vec<Value> = serde_json::from_str(&batch.expect("stream open")).unwrap();
            let from = self.seen.len();
            self.seen.extend(batch);
            if let Some(i) = self.seen[from..].iter().position(&want) {
                return from + i;
            }
        }
    }

    async fn done(&mut self, call: u64) -> (usize, Value) {
        let i = self.until(&format!("done {call}"), |r| r["type"] == "done" && r["call"] == call).await;
        (i, self.seen[i]["result"].clone())
    }

    /// Creates a bot on `server`; returns its id.
    async fn create(&mut self, server: &FakeServer, call: u64, name: &str) -> String {
        let addr = server.addr();
        let params = json!({ "host": addr.ip().to_string(), "port": addr.port(), "username": name, "transport": "raknet", "events": ["chat"] });
        self.call(call, "createBot", params);
        let (i, result) = self.done(call).await;
        let bot = result["ok"]["bot"].as_str().unwrap_or_else(|| panic!("createBot failed: {result}")).to_owned();
        let spawn = self.until("spawn", |r| r["type"] == "event" && r["event"]["type"] == "spawn" && r["bot"] == bot).await;
        assert!(spawn < i, "spawn comes before createBot resolves");
        server.spawned().await.expect("script reaches PlayerSpawn");
        bot
    }

    fn event_index(&self, bot: &str, event: impl Fn(&Value) -> bool) -> Option<usize> {
        self.seen.iter().position(|r| r["type"] == "event" && r["bot"] == bot && event(&r["event"]))
    }
}

fn chat_line(message: &str) -> Text {
    Text {
        needs_translation: false,
        category: TextCategory::MessageOnly,
        r#type: TextType::Raw,
        content: TextContent::Raw(TextContentRaw { message: message.into() }),
        xuid: String::new(),
        platform_chat_id: String::new(),
        has_filtered_message: false,
        filtered_message: None,
    }
}

fn code(result: &Value) -> &str {
    result["err"]["code"].as_str().unwrap_or_else(|| panic!("not an error: {result}"))
}

fn sent_chat(server: &FakeServer, text: &str) -> bool {
    let lines = server.received().into_iter().filter_map(|r| r.packet.decode::<Text>().ok());
    lines.into_iter().any(|t| matches!(&t.content, TextContent::Chat(c) if c.message == text))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_bot_runs_on_json_alone() {
    let server = FakeServer::start(Script::bds_spawn()).await.unwrap();
    let mut d = Driver::start();
    let bot = d.create(&server, 1, "HostTester").await;

    d.call(2, "bot.chat", json!({ "bot": bot, "message": "hello" }));
    assert_eq!(d.done(2).await.1, json!({ "ok": null }));
    let deadline = Instant::now() + Duration::from_secs(5);
    while !sent_chat(&server, "hello") {
        assert!(Instant::now() < deadline, "the chat line never reached the server");
        sleep(Duration::from_millis(10)).await;
    }

    d.call(3, "bot.waitForTicks", json!({ "bot": bot, "ticks": 30 }));
    sleep(Duration::from_millis(400)).await;
    server.send(&chat_line("mid-action"));
    d.call(4, "bot.snapshot", json!({ "bot": bot }));
    let (snapshot_at, snapshot) = d.done(4).await;
    assert_eq!(snapshot["ok"]["hot"]["alive"], true, "{snapshot}");
    let hot = d.host.hot(&BotId(bot.clone())).expect("a spawned bot publishes its state");
    assert!(hot.alive && hot.tick > 0, "{hot:?}");
    assert!(d.host.hot(&BotId("bot-9".into())).is_none());
    let feet = BlockPos { x: hot.position.x.floor() as i32, y: hot.position.y.floor() as i32 - 1, z: hot.position.z.floor() as i32 };
    let deadline = Instant::now() + Duration::from_secs(5);
    let block = loop {
        match d.host.block_at(&BotId(bot.clone()), feet) {
            Some(block) => break block,
            None => assert!(Instant::now() < deadline, "the chunk at {feet:?} never loaded"),
        }
        sleep(Duration::from_millis(50)).await;
    };
    assert!(block.name.starts_with("minecraft:"), "{block:?}");
    let (done_at, waited) = d.done(3).await;
    assert_eq!(waited, json!({ "ok": null }));
    assert!(snapshot_at < done_at, "an immediate call answers while a queued one runs");
    let chat = d.event_index(&bot, |e| e["type"] == "chat" && e["plain"] == "mid-action").expect("the chat event");
    assert!(chat < done_at, "the chat event raised during the wait comes before its done");

    d.call(5, "bot.waitForTicks", json!({ "bot": bot, "ticks": 400 }));
    d.call(6, "bot.waitForTicks", json!({ "bot": bot, "ticks": 400 }));
    d.call(7, "bot.waitForTicks", json!({ "bot": bot, "ticks": 2 }));
    d.call(8, "cancel", json!({ "bot": bot, "call": 6 }));
    let (queued_at, queued) = d.done(6).await;
    assert_eq!(code(&queued), "CANCELLED", "a queued call is removed");
    sleep(Duration::from_millis(300)).await;
    let cancelled = Instant::now();
    d.call(9, "cancel", json!({ "bot": bot, "call": 5 }));
    let (running_at, running) = d.done(5).await;
    assert_eq!(code(&running), "CANCELLED");
    assert!(cancelled.elapsed() < Duration::from_secs(1), "cancel took {:?}", cancelled.elapsed());
    assert!(queued_at < running_at);
    assert_eq!(d.done(7).await.1, json!({ "ok": null }), "the next call runs after a cancel");
    assert_eq!(d.done(8).await.1, json!({ "ok": null }));
    assert_eq!(d.done(9).await.1, json!({ "ok": null }));

    d.call(10, "bot.waitForTicks", json!({ "bot": bot, "ticks": 400 }));
    d.call(11, "bot.waitForTicks", json!({ "bot": bot, "ticks": 1 }));
    d.call(12, "bot.close", json!({ "bot": bot }));
    let (closed_at, closed) = d.done(12).await;
    assert_eq!(closed, json!({ "ok": null }));
    assert_eq!(code(&d.done(10).await.1), "CANCELLED", "close cancels the running call");
    assert_eq!(code(&d.done(11).await.1), "DISCONNECTED", "and fails the queued ones");
    let end = d.event_index(&bot, |e| e["type"] == "end").expect("an end event");
    assert_eq!(d.seen[end]["event"]["reason"], "closed");
    assert!(end < closed_at, "end comes before close resolves");

    let after = json!({ "call": 13, "method": "bot.chat", "params": { "bot": bot, "message": "late" } }).to_string();
    assert_eq!(d.host.call(&after).unwrap_err().code, ErrorCode::UnknownBot);
    let seqs: Vec<u64> = d.seen.iter().filter(|r| r["type"] == "event").map(|r| r["seq"].as_u64().unwrap()).collect();
    assert!(seqs.windows(2).all(|w| w[1] == w[0] + 1) && seqs[0] == 1, "seq counts every event: {seqs:?}");
    d.host.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_kick_fails_the_running_and_queued_calls() {
    let server = FakeServer::start(Script::bds_spawn()).await.unwrap();
    let mut d = Driver::start();
    let bot = d.create(&server, 1, "KickTester").await;
    d.call(2, "bot.waitForTicks", json!({ "bot": bot, "ticks": 400 }));
    d.call(3, "bot.waitForTicks", json!({ "bot": bot, "ticks": 1 }));
    sleep(Duration::from_millis(300)).await;
    server.kick("bye");
    let (running_at, running) = d.done(2).await;
    assert_eq!(code(&running), "DISCONNECTED");
    assert_eq!(code(&d.done(3).await.1), "DISCONNECTED");
    let end = d.event_index(&bot, |e| e["type"] == "end").expect("an end event");
    assert_eq!(d.seen[end]["event"]["reason"], "kicked", "{}", d.seen[end]);
    assert!(end < running_at, "end comes before the failed call's done");
    d.host.shutdown().await;
    while d.out.recv().await.is_some() {}
}

#[tokio::test]
async fn refusals_come_back_at_once() {
    let (host, _out) = Host::start(HostConfig { shards: 1, ..HostConfig::default() }).unwrap();
    assert_eq!(host.call("{").unwrap_err().code, ErrorCode::BadParams);
    assert_eq!(host.call(r#"{"call": 1, "method": "bot.jump", "params": {}}"#).unwrap_err().code, ErrorCode::UnknownMethod);
    let chat = r#"{"call": 2, "method": "bot.chat", "params": {"bot": "bot-9", "message": "hi"}}"#;
    assert_eq!(host.call(chat).unwrap_err().code, ErrorCode::UnknownBot);
    host.shutdown().await;
    let create = r#"{"call": 3, "method": "createBot", "params": {"host": "127.0.0.1", "username": "Late"}}"#;
    assert_eq!(host.call(create).unwrap_err().code, ErrorCode::Closed);
}
