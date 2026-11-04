//! One bot's task on its shard: connect, then run queued calls one at a time while immediate calls, events and
//! snapshots keep flowing (contract: the crate docs).

mod join;
mod lanes;

use std::collections::VecDeque;
use std::sync::Arc;

use acacia_api::{ApiError, Auth, CallId, CreateBotParams, CreateBotResult, EndReason, ErrorCode, Event, Method, Reply, Transport};
use acacia_bot::client::{Client, TransportKind};
use acacia_bot::events::channel;
use acacia_bot::state::Trackers;
use acacia_bot::swarm::JoinQueue;
use acacia_bot::world::SharedWorlds;
use acacia_bot::{Bot, BotConfig};
use parking_lot::Mutex;
use tokio::sync::mpsc;

use crate::calls::{action, Cancellable};
use crate::convert;
use crate::registry::{BotOut, Msg, Registry};
use crate::sign_in::SignIns;
use lanes::Lanes;

/// What a new bot's task starts from.
pub(crate) struct Spawn {
    pub out: Arc<BotOut>,
    pub rx: mpsc::UnboundedReceiver<Msg>,
    pub registry: Arc<Mutex<Registry>>,
    /// The `createBot` call.
    pub call: CallId,
    pub params: CreateBotParams,
    pub shared_worlds: SharedWorlds,
    /// Shared by every bot of the host.
    pub joins: Arc<JoinQueue>,
    pub sign_ins: Arc<SignIns>,
    pub event_capacity: usize,
    pub snapshots: bool,
    pub max_batch_bytes: usize,
}

pub(crate) async fn run(spawn: Spawn) {
    let Spawn { out, mut rx, registry, call, params, shared_worlds, joins, sign_ins, event_capacity, snapshots, max_batch_bytes } = spawn;
    let (sink, events) = channel(event_capacity);
    let transport = match params.transport.unwrap_or(Transport::Auto) {
        Transport::Auto => TransportKind::Auto,
        Transport::Raknet => TransportKind::RakNet,
        Transport::Nethernet => TransportKind::NetherNet,
    };
    let address = format!("{}:{}", params.host, params.port.unwrap_or(19132));
    let config = BotConfig {
        trackers: Trackers { entities: params.entities.unwrap_or(false), ..Trackers::default() },
        physics: params.physics.unwrap_or(false),
        events: convert::events(params.events.as_deref().unwrap_or_default()),
        event_sink: Some(sink),
        snapshots,
        shared_worlds,
        ..BotConfig::default()
    };
    let mut early = VecDeque::new();
    let connected = {
        let connect = async {
            let login = match params.auth.unwrap_or(Auth::Offline) {
                Auth::Offline => None,
                Auth::Microsoft => {
                    let account = sign_ins.account(&params.username, params.profiles_folder.as_deref());
                    let prompt = out.clone();
                    Some(sign_ins.login(account, Box::new(move |code| prompt.msa_code(call, code))).await?)
                }
            };
            let builder = || {
                let builder = Client::builder(&address).transport(transport).max_batch_bytes(max_batch_bytes);
                match &login {
                    Some(l) => builder.online(l.credentials.clone(), l.key.clone()),
                    None => builder.offline(&params.username),
                }
            };
            join::connect(&joins, &address, builder, &config).await.map_err(convert::connect_error)
        };
        tokio::pin!(connect);
        loop {
            tokio::select! {
                bot = &mut connect => break bot,
                msg = rx.recv() => match msg {
                    Some(Msg::Call(c, Method::Cancel(p))) if p.call == call => {
                        out.done(c, Ok(Reply::Cancel(())));
                        break Err(ApiError::new(ErrorCode::Cancelled, "cancelled while connecting"));
                    }
                    Some(Msg::Call(c, close @ Method::Close(_))) => {
                        early.push_back(Msg::Call(c, close));
                        break Err(ApiError::new(ErrorCode::Cancelled, "closed while connecting"));
                    }
                    Some(Msg::Shutdown) | None => break Err(ApiError::new(ErrorCode::Closed, "the host is shutting down")),
                    Some(msg) => early.push_back(msg),
                },
            }
        }
    };
    let bot = match connected {
        Ok(bot) => bot,
        Err(e) => {
            out.done(call, Err(e));
            registry.lock().remove(&out.id);
            answer_after_end(&out, early.into_iter().chain(std::iter::from_fn(|| rx.try_recv().ok())));
            return;
        }
    };
    registry.lock().set_reader(&out.id, bot.snapshot_reader());
    out.event(Event::Spawn);
    out.done(call, Ok(Reply::CreateBot(CreateBotResult { bot: out.id.clone() })));
    let lanes = Lanes {
        out: out.clone(),
        rx,
        events,
        sender: bot.sender(),
        inbox: bot.inbox(),
        reader: bot.snapshot_reader(),
        queue: VecDeque::new(),
        snapshots: VecDeque::new(),
        closing: false,
        closers: Vec::new(),
        ended: false,
    };
    let mut lanes = play(bot, lanes, early).await;
    // Out of the registry before `bot.close` resolves, so later calls are refused `UNKNOWN_BOT` at once.
    registry.lock().remove(&out.id);
    for call in lanes.closers.drain(..) {
        out.done(call, Ok(Reply::Close(())));
    }
    answer_after_end(&out, std::iter::from_fn(|| lanes.rx.try_recv().ok()));
}

/// Runs the bot until it closes or disconnects, then ends it; returns the lanes for the last stragglers.
async fn play(mut bot: Bot, mut lanes: Lanes, early: VecDeque<Msg>) -> Lanes {
    for msg in early {
        lanes.on_wake(lanes::Wake::Msg(Some(msg)), None);
    }
    let cancel = bot.cancel_handle();
    while !lanes.closing && bot.disconnect_reason().is_none() {
        if let Some((call, method)) = lanes.queue.pop_front() {
            let mut running = Cancellable::new(action(&mut bot, method), cancel.clone());
            let result = loop {
                tokio::select! {
                    wake = lanes.wake() => if lanes.on_wake(wake, Some(call)) { running.cancel() },
                    result = &mut running => break result,
                }
            };
            drop(running);
            // The call's done comes after every event it raised.
            lanes.drain_events();
            lanes.out.done(call, result);
            bot.abandon_action();
            continue;
        }
        // Idle: keep the bot polled (auto-eat runs only in `next`).
        tokio::select! {
            wake = lanes.wake() => {
                lanes.on_wake(wake, None);
            }
            alive = bot.next() => if alive.is_none() { break },
        }
    }
    if bot.disconnect_reason().is_none() {
        bot.disconnect().await;
    }
    lanes.drain_events();
    if !lanes.ended {
        lanes.out.event(Event::End { reason: EndReason::Closed, message: "closed".into() });
    }
    lanes.fail_open();
    lanes
}

/// Answers calls that reached a bot after it ended.
fn answer_after_end(out: &BotOut, msgs: impl Iterator<Item = Msg>) {
    for msg in msgs {
        if let Msg::Call(call, method) = msg {
            match method {
                Method::Close(_) => out.done(call, Ok(Reply::Close(()))),
                _ => out.fail(call, ErrorCode::Disconnected, "the bot has ended"),
            }
        }
    }
}
