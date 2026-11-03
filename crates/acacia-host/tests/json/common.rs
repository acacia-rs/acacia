//! Driving a host purely through command JSON and reading its record stream back.
#![allow(dead_code)]

use std::time::{Duration, Instant};

use acacia_api::{BlockPos, BotId, HotSnapshot};
use acacia_bot::proto::packets::{Text, TextCategory, TextContent, TextContentRaw, TextType};
use acacia_host::{Host, HostConfig, Outbound};
use acacia_testserver::{bds_block_id, FakeServer};
use serde_json::{json, Value};
use tokio::time::{sleep, timeout};

pub struct Driver {
    pub host: Host,
    pub out: Outbound,
    /// Every record read so far, in stream order.
    pub seen: Vec<Value>,
}

impl Driver {
    pub fn start() -> Self {
        let (host, out) = Host::start(HostConfig { shards: 2, ..HostConfig::default() }).unwrap();
        Self { host, out, seen: Vec::new() }
    }

    pub fn call(&self, call: u64, method: &str, params: Value) {
        let command = json!({ "call": call, "method": method, "params": params }).to_string();
        self.host.call(&command).unwrap_or_else(|e| panic!("{method} refused: {e}"));
    }

    /// Reads the stream until a record matches `want`; returns its index in `seen`.
    pub async fn until(&mut self, what: &str, want: impl Fn(&Value) -> bool) -> usize {
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

    pub async fn done(&mut self, call: u64) -> (usize, Value) {
        let i = self.until(&format!("done {call}"), |r| r["type"] == "done" && r["call"] == call).await;
        (i, self.seen[i]["result"].clone())
    }

    /// Calls and waits for the result, which must be `ok`.
    pub async fn ok(&mut self, call: u64, method: &str, params: Value) -> Value {
        self.call(call, method, params);
        self.ok_done(call).await
    }

    /// Waits for call `call`'s result, which must be `ok`.
    pub async fn ok_done(&mut self, call: u64) -> Value {
        let (_, result) = self.done(call).await;
        result.get("ok").unwrap_or_else(|| panic!("call {call} failed: {result}")).clone()
    }

    /// Creates a bot on `server` (chat events on, plus `extra` params); returns its id.
    pub async fn create_with(&mut self, server: &FakeServer, call: u64, name: &str, extra: Value) -> String {
        let addr = server.addr();
        let mut params = json!({ "host": addr.ip().to_string(), "port": addr.port(), "username": name, "transport": "raknet", "events": ["chat"] });
        params.as_object_mut().unwrap().extend(extra.as_object().cloned().unwrap_or_default());
        self.call(call, "createBot", params);
        let (i, result) = self.done(call).await;
        let bot = result["ok"]["bot"].as_str().unwrap_or_else(|| panic!("createBot failed: {result}")).to_owned();
        let spawn = self.until("spawn", |r| r["type"] == "event" && r["event"]["type"] == "spawn" && r["bot"] == bot).await;
        assert!(spawn < i, "spawn comes before createBot resolves");
        server.spawned().await.expect("script reaches PlayerSpawn");
        bot
    }

    pub async fn create(&mut self, server: &FakeServer, call: u64, name: &str) -> String {
        self.create_with(server, call, name, json!({})).await
    }

    pub fn event_index(&self, bot: &str, event: impl Fn(&Value) -> bool) -> Option<usize> {
        self.seen.iter().position(|r| r["type"] == "event" && r["bot"] == bot && event(&r["event"]))
    }

    /// Reads the stream until the bot's event matches; returns the event.
    pub async fn event(&mut self, bot: &str, what: &str, want: impl Fn(&Value) -> bool) -> Value {
        let i = self.until(what, |r| r["type"] == "event" && r["bot"] == bot && want(&r["event"])).await;
        self.seen[i]["event"].clone()
    }

    /// Polls a sync read until it yields.
    pub async fn read<T>(&self, what: &str, read: impl Fn(&Host) -> Option<T>) -> T {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if let Some(value) = read(&self.host) {
                return value;
            }
            assert!(Instant::now() < deadline, "timed out waiting for {what}");
            sleep(Duration::from_millis(20)).await;
        }
    }

    /// Lays a stone floor under a physics bot's spawn once the chunks are there, and waits until it stands on it;
    /// returns its feet cell.
    pub async fn stand_on_floor(&self, server: &FakeServer, bot: &str, half: i32) -> BlockPos {
        let id = BotId(bot.to_owned());
        let [x, y, z] = server.spawn_feet().expect("the script's StartGame");
        let feet = [x.floor() as i32, y.floor() as i32, z.floor() as i32];
        let cells = (-half..=half).flat_map(|dx| (-half..=half).map(move |dz| BlockPos { x: feet[0] + dx, y: feet[1] - 1, z: feet[2] + dz }));
        let cells: Vec<_> = cells.collect();
        self.read("the floor's chunks", |h| cells.iter().all(|c| h.block_at(&id, *c).is_some()).then_some(())).await;
        server.lay_floor(feet, half, bds_block_id("minecraft:stone"));
        let under = cells[cells.len() / 2];
        let deadline = Instant::now() + Duration::from_secs(10);
        let shown = loop {
            let shown = self.host.block_at(&id, under);
            if shown.as_ref().is_some_and(|b| b.name == "minecraft:stone") {
                break shown;
            }
            assert!(Instant::now() < deadline, "the floor at {under:?} shows {shown:?}");
            sleep(Duration::from_millis(20)).await;
        };
        let hot = loop {
            let hot = self.hot(bot);
            if hot.on_ground {
                break hot;
            }
            assert!(Instant::now() < deadline, "the bot is not standing on {shown:?} at {under:?}: {hot:?}");
            sleep(Duration::from_millis(20)).await;
        };
        BlockPos { x: hot.position.x.floor() as i32, y: (hot.position.y + 0.01).floor() as i32, z: hot.position.z.floor() as i32 }
    }

    pub async fn hot_until(&self, bot: &str, what: &str, want: impl Fn(&HotSnapshot) -> bool) -> HotSnapshot {
        let id = BotId(bot.to_owned());
        self.read(what, |h| h.hot(&id).filter(&want)).await
    }

    /// Waits until the bot's published entity list holds runtime id `id`.
    pub async fn tracked(&self, bot: &str, id: u64) {
        let bot = BotId(bot.to_owned());
        self.read(&format!("entity {id} tracked"), |h| h.entities(&bot).filter(|l| l.entities.iter().any(|e| e.id.0 == id))).await;
    }

    /// Waits until the bot's published inventory holds exactly `count` of item `name` in all.
    pub async fn holding(&self, server: &FakeServer, bot: &str, name: &str, count: u32) {
        let (id, bot) = (i32::from(server.item_id(name).expect("an item the script knows")), BotId(bot.to_owned()));
        let held = |h: &Host| h.cold(&bot).map_or(0, |c| c.inventory.iter().filter(|s| s.network_id == id).map(|s| u32::from(s.count)).sum());
        self.read(&format!("{count} {name}"), |h| (held(h) == count).then_some(())).await;
    }

    pub fn hot(&self, bot: &str) -> HotSnapshot {
        self.host.hot(&BotId(bot.to_owned())).expect("a spawned bot publishes its state")
    }
}

pub fn chat_line(message: &str) -> Text {
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

pub fn code(result: &Value) -> &str {
    result["err"]["code"].as_str().unwrap_or_else(|| panic!("not an error: {result}"))
}

/// Waits until the server has received a packet with id `id`.
pub async fn received(server: &FakeServer, id: u32) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !server.received().iter().any(|r| r.packet.id == id) {
        assert!(Instant::now() < deadline, "the server never got packet {id}");
        sleep(Duration::from_millis(20)).await;
    }
}

pub fn sent_chat(server: &FakeServer, text: &str) -> bool {
    let lines = server.received().into_iter().filter_map(|r| r.packet.decode::<Text>().ok());
    lines.into_iter().any(|t| matches!(&t.content, TextContent::Chat(c) if c.message == text))
}
