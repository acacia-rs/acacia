//! Runs acacia bots for a transport: [`acacia_api::Command`] JSON in, one ordered stream of [`acacia_api::Out`]
//! batches out. The napi addon and a WebSocket server both sit on it and add only framing. The one place the
//! host's contract is written down (the wire format itself: acacia-api's crate docs):
//!
//! - **Threads.** [`Host`] owns K shard threads ([`acacia_bot::swarm::Shard`]: a current-thread runtime and a
//!   `LocalSet` each). A new bot goes to the shard with the fewest bots and never moves.
//! - **Calls.** [`Host::call`] takes a command from any thread and only enqueues it. A refusal (bad JSON, unknown
//!   bot, `BUSY` past [`HostConfig::max_open_calls`], `CLOSED`) comes back at once and gets no `done`.
//! - **Per bot**, one task owns the bot. Queued calls run one at a time in order, and each is followed by
//!   `abandon_action` (acacia_bot::cancel), so a cancelled or failed call leaves nothing held. Immediate calls go
//!   through the bot's `ClientSender` and `Inbox`, so they run while a queued call does. Between calls the task
//!   keeps polling `Bot::next` (auto-eat needs it). `cancel` removes a queued call or cancels the running one,
//!   re-firing the cancel until the action has started waiting.
//! - **Stream.** The bot's events leave through its event sink as they happen (`acacia_bot::events::channel`),
//!   are converted and numbered, and a call's `done` is written only after the events raised before it ended.
//!   Each shard buffers its bots' records and sends them as one JSON array at most once per
//!   [`HostConfig::flush_every`], or at once when a `done` is buffered. When the outbound channel is full the shard keeps them and merges them into
//!   its next batch, dropping lossy events (counted in `lagged`) past [`HostConfig::max_held`].
//! - **Lifecycle.** `createBot` answers once the bot has spawned, after a `spawn` event. `bot.close` cancels the
//!   running call, disconnects (at most 2 s), emits `end`, fails queued calls `DISCONNECTED` and then answers.
//!   A server disconnect fails the running call (`DISCONNECTED` from the action) and the queued ones the same
//!   way, after `end`. A panicking bot task emits `error`, fails its open calls `INTERNAL` and ends; the shard
//!   lives on. [`Host::shutdown`] closes every bot, flushes and joins the shards.
//! - **Consumers.** [`Outbound`] yields each batch as a `String`: a napi addon hands one per call to its sink
//!   threadsafe function from a thread blocked in [`Outbound::blocking_recv`]; a socket server sends each as a
//!   frame. The same bytes either way.
//!
//! - **Reads.** With [`HostConfig::snapshots`] each bot publishes its state per tick (acacia_bot::snapshot), and
//!   [`Host::hot`], [`Host::cold`], [`Host::entities`] and [`Host::block_at`] read it and the bot's terrain from
//!   any thread without a call or a `done`. [`Host::entity_frame`] is the entity list for per-tick readers.
//!
//! Not yet: reconnects through the swarm supervisor, online sign-in.

mod bot;
mod calls;
mod convert;
mod host;
mod outbound;
mod reads;
mod registry;

pub use host::{Host, HostConfig};
pub use outbound::Outbound;
pub use reads::{EntityFrame, ENTITY_NUMBERS};
