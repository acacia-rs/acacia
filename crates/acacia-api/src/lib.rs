//! The schema every acacia transport speaks (the napi addon now, a WebSocket server and MCP later): plain serde
//! values with no dependency on the bot stack, so exporting the types never builds it. The one place the wire
//! contract is written down:
//!
//! - **Values.** Fields are camelCase; enums are objects tagged by `type`; 64-bit ids ([`EntityId`],
//!   [`UniqueId`]) are decimal strings, other counters plain numbers; positions are `{x, y, z}`; [`BotId`] is a
//!   string the host picks; [`CallId`] is a number the caller picks.
//! - **Calls.** A [`Command`] is `{call, method, params}` with the method and params from the [`METHODS`] table.
//!   A command that does not parse, or that the host cannot take (`BUSY`, `UNKNOWN_BOT`), is refused at once;
//!   every other call ends with exactly one [`Out::Done`] in the stream. [`Lane`] says how the host runs it:
//!   queued calls take the bot one at a time in order, immediate ones run at once, mid-action too.
//! - **Stream.** Everything else comes back as [`Out`] records, moved in batches (a JSON array, [`batch_json`]).
//!   Per bot, records keep the order the bot produced them in, and a call's `done` comes after every event the
//!   bot raised before the call ended, so a caller sees what happened during an action before it settles.
//!   `seq` numbers each bot's events. Lossy events ([`Event::is_lossy`]) may be dropped under backpressure and
//!   are then counted in a `lagged` record; nothing else is dropped.
//! - **Sync reads.** Transports that can read a bot's published state without a call ([`HotSnapshot`],
//!   [`ColdSnapshot`], [`EntityList`], [`Block`]) hand out these types; they are in `generated/types` though no
//!   method returns them.
//! - **Versions.** [`PROTOCOL_VERSION`] changes only on a breaking change; within it, changes are additive (new
//!   methods, events and optional fields), so consumers keep a catch-all arm.
//! - **Generated files.** `generated/methods.json` (each method's JSON Schemas, for MCP), `generated/methods.ts`
//!   (the typed `Methods` map) and `generated/types/*.ts` (ts-rs) are committed. `tests/generated.rs` fails when
//!   they drift; `ACACIA_API_REGEN=1 cargo test -p acacia-api --test generated` rewrites them.

mod entity;
mod error;
mod event;
pub mod export;
mod form;
mod geom;
mod ids;
mod methods;
mod out;
pub mod params;
mod snapshot;
pub mod table;

pub use entity::{Entity, EntityList};
pub use error::{ApiError, ErrorCode};
pub use event::{ChatKind, ChatLine, EndReason, Event, TitleKind, Window};
pub use form::{Form, FormAnswer, FormBody, FormButton, FormElement, FormValue};
pub use geom::{Block, BlockPos, Vec3};
pub use ids::{BotId, CallId, EntityId, UniqueId};
pub use methods::*;
pub use out::{batch_json, CallResult, Command, Out};
pub use params::*;
pub use snapshot::{ColdSnapshot, Effect, HotSnapshot, PlayerInfo, Snapshot, Stack};
pub use table::{Lane, MethodInfo};

/// Bumped only by a breaking change to the schema.
pub const PROTOCOL_VERSION: u32 = 1;
