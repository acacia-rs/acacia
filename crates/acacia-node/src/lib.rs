//! The Node-API addon under `@acacia-rs/bot` (packages/acacia-bot): an acacia-host [`acacia_host::Host`] moved
//! across the napi boundary as JSON strings, nothing typed on this side. The one place the native contract is
//! written down (the wire: acacia-api's crate docs; the host: acacia-host's; the TS layer: the package README):
//!
//! - **`new NativeHost({ shards?, onRecords })`** starts the host. A forwarder thread blocks on its record stream
//!   and hands each batch (a JSON array of `Out` records) to `onRecords` in one call, through one weak NonBlocking
//!   threadsafe function. When the queue is full the forwarder keeps the batch and merges the next ones into it,
//!   so nothing is dropped here and per-bot order holds. After the host has shut down and the last batch is out,
//!   `onRecords(null)` ends the stream. `onRecords` must not throw: a throw is an uncaught exception.
//! - **`call(json)`** enqueues one `Command` and returns at once. A refusal throws an `Error` whose `code` is the
//!   acacia-api `ErrorCode` (`BUSY`, `UNKNOWN_BOT`, `BAD_PARAMS`, ...); that call gets no `done`.
//! - **`close()`** starts the shutdown on a thread of its own and returns; the stream ends with `onRecords(null)`.
//! - **Keep-alive.** The threadsafe function is weak, so the addon never holds Node's event loop by itself.
//!   `setKeepAlive(true)` refs it and `false` unrefs it; the caller holds it while it owes the script something
//!   (live bots, unanswered calls, a close in progress) and must release it once the stream has ended.
//! - **Sync reads** (`snapshotHot`, `snapshotCold`, `snapshotEntities`, `blockAt`) return acacia-api
//!   `HotSnapshot` / `ColdSnapshot` / `EntityList` / `Block` JSON, or `null` (unknown bot, no tick yet, chunk not
//!   loaded), without a call. `snapshotHot` serializes at most once per bot tick, so callers can compare strings
//!   to skip a parse; the versioned parts have `coldVersion` / `entitiesVersion` to check before fetching.
//!   `entityDelta(bot, since, numbers)` is the entity list as changes since the caller's last version (acacia-host's
//!   `EntityDelta`): entities that came or went as JSON, numbers written straight into the caller's array by slot.
//!   `itemTable` is the server's item names (fetch once it is non-empty); `findBlocks` runs `bot.findBlocks`
//!   synchronously (bounded by its caps) and throws with the error's `code`.
//! - **Panics** in the sync functions become JS errors (`catch_unwind`); a bot task's panic is the host's
//!   (`error` event, its calls fail `INTERNAL`).
//! - **`TestServer`** (feature `test-server`, test builds only) runs acacia-testserver's `FakeServer` for the
//!   package's end-to-end tests. In those builds `ACACIA_FAKE_SIGN_IN=1` at `new NativeHost` swaps Microsoft
//!   sign-in for acacia-host's `FakeSignIn` (code `FAKE1234`, signed in 300 ms after the prompt, nothing on disk);
//!   a value over 1 is that many ms instead.

mod error;
mod forward;
mod host;
#[cfg(feature = "test-server")]
mod test_server;
