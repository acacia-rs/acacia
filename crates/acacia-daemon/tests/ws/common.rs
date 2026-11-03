//! A JSON-RPC WebSocket client for driving the daemon, and FakeServer helpers.
#![allow(dead_code)]

use std::net::SocketAddr;
use std::time::{Duration, Instant};

use acacia_daemon::{Daemon, DaemonConfig};
use acacia_host::HostConfig;
use acacia_proto::packets::{Text, TextCategory, TextContent, TextContentChat, TextType};
use acacia_testserver::FakeServer;
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::net::TcpStream;
use tokio::time::{sleep, timeout};
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::{self, Message};
use tokio_tungstenite::WebSocketStream;

pub async fn daemon(token: Option<&str>) -> Daemon {
    let config = DaemonConfig { host: HostConfig { shards: 1, ..HostConfig::default() }, token: token.map(Into::into), ..DaemonConfig::default() };
    Daemon::bind("127.0.0.1:0", config).await.unwrap()
}

pub async fn handshake(addr: SocketAddr, origin: Option<&str>) -> Result<WebSocketStream<TcpStream>, tungstenite::Error> {
    let mut request = format!("ws://{addr}/").into_client_request().unwrap();
    if let Some(origin) = origin {
        request.headers_mut().insert("origin", origin.parse().unwrap());
    }
    let tcp = TcpStream::connect(addr).await.unwrap();
    Ok(tokio_tungstenite::client_async(request, tcp).await?.0)
}

pub struct Client {
    ws: WebSocketStream<TcpStream>,
    /// Responses and `out` records (each record on its own), in arrival order.
    pub seen: Vec<Value>,
    next: u64,
    pub closed: bool,
}

impl Client {
    pub async fn connect(addr: SocketAddr) -> Self {
        Self { ws: handshake(addr, None).await.unwrap(), seen: Vec::new(), next: 100, closed: false }
    }

    /// Connects and says hello (protocol 1, mirror on, plus `extra`); panics unless it is accepted.
    pub async fn greeted(addr: SocketAddr, extra: Value) -> Self {
        let mut client = Self::connect(addr).await;
        let mut params = json!({ "protocolVersion": acacia_api::PROTOCOL_VERSION, "client": "test", "mirror": true });
        params.as_object_mut().unwrap().extend(extra.as_object().cloned().unwrap_or_default());
        client.ok("hello", params).await;
        client
    }

    pub async fn send_text(&mut self, text: &str) {
        self.ws.send(Message::text(text)).await.unwrap();
    }

    pub async fn request(&mut self, method: &str, params: Value) -> u64 {
        self.next += 1;
        let frame = json!({ "jsonrpc": "2.0", "id": self.next, "method": method, "params": params });
        self.send_text(&frame.to_string()).await;
        self.next
    }

    /// Reads frames until one matches `want`; returns its index in `seen`.
    pub async fn until(&mut self, what: &str, want: impl Fn(&Value) -> bool) -> usize {
        if let Some(i) = self.seen.iter().position(&want) {
            return i;
        }
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            let from = self.seen.len();
            self.read_frame(what, deadline).await;
            if let Some(i) = self.seen[from..].iter().position(&want) {
                return from + i;
            }
        }
    }

    /// Reads one frame into `seen`; panics at `deadline` or when the socket closes.
    pub async fn read_frame(&mut self, what: &str, deadline: Instant) {
        let left = deadline.saturating_duration_since(Instant::now());
        let frame = timeout(left, self.ws.next()).await.unwrap_or_else(|_| panic!("no {what}; seen {:#?}", self.seen));
        let Some(Ok(Message::Text(text))) = frame else {
            self.closed = true;
            panic!("the socket closed before {what}: {frame:?}");
        };
        let frame: Value = serde_json::from_str(text.as_str()).unwrap();
        match frame["method"].as_str() {
            Some("out") => self.seen.extend(frame["params"]["records"].as_array().unwrap().iter().cloned()),
            _ => self.seen.push(frame),
        }
    }

    pub async fn response(&mut self, id: u64) -> (usize, Value) {
        let i = self.until(&format!("response {id}"), |f| f["id"] == id).await;
        (i, self.seen[i].clone())
    }

    /// Calls and waits for the result, which must not be an error.
    pub async fn ok(&mut self, method: &str, params: Value) -> Value {
        let id = self.request(method, params).await;
        let (_, frame) = self.response(id).await;
        frame.get("result").unwrap_or_else(|| panic!("{method} failed: {frame}")).clone()
    }

    /// Calls and returns the error's `data.code`, which must be there.
    pub async fn err(&mut self, method: &str, params: Value) -> String {
        let id = self.request(method, params).await;
        error_code(&self.response(id).await.1)
    }

    pub async fn create(&mut self, server: &FakeServer, name: &str, extra: Value) -> String {
        let addr = server.addr();
        let mut params = json!({ "host": addr.ip().to_string(), "port": addr.port(), "username": name, "transport": "raknet", "events": ["chat"] });
        params.as_object_mut().unwrap().extend(extra.as_object().cloned().unwrap_or_default());
        let result = self.ok("createBot", params).await;
        server.spawned().await.expect("script reaches PlayerSpawn");
        result["bot"].as_str().unwrap().to_owned()
    }

    pub fn event_index(&self, bot: &str, want: impl Fn(&Value) -> bool) -> Option<usize> {
        self.seen.iter().position(|r| r["type"] == "event" && r["bot"] == bot && want(&r["event"]))
    }

    /// Waits until the daemon closes the socket.
    pub async fn wait_closed(&mut self) {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            match timeout(left, self.ws.next()).await.expect("the daemon keeps the socket open") {
                Some(Ok(Message::Text(_))) | Some(Ok(Message::Ping(_) | Message::Pong(_))) => continue,
                _ => return,
            }
        }
    }

    pub async fn close(mut self) {
        let _ = self.ws.close(None).await;
    }
}

pub fn error_code(frame: &Value) -> String {
    frame["error"]["data"]["code"].as_str().unwrap_or_else(|| panic!("not an error: {frame}")).to_owned()
}

pub fn chat(server: &FakeServer, sender: &str, message: &str) {
    server.send(&Text {
        needs_translation: false,
        category: TextCategory::Authored,
        r#type: TextType::Chat,
        content: TextContent::Chat(TextContentChat { source_name: sender.into(), message: message.into() }),
        xuid: String::new(),
        platform_chat_id: String::new(),
        has_filtered_message: false,
        filtered_message: None,
    });
}

pub async fn server_got_chat(server: &FakeServer, message: &str) {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let lines = server.received().into_iter().filter_map(|r| r.packet.decode::<Text>().ok());
        if lines.into_iter().any(|t| matches!(&t.content, TextContent::Chat(c) if c.message == message)) {
            return;
        }
        assert!(Instant::now() < deadline, "the server never got {message:?}");
        sleep(Duration::from_millis(20)).await;
    }
}
