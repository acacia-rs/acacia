use std::sync::Arc;
use std::time::Duration;

use acacia_api::{ApiError, BlockAtCursorParams, BlockPos, BotId, ErrorCode, FindBlocksParams};
use serde::de::DeserializeOwned;
use serde::Serialize;
use acacia_host::{Host, HostConfig, ENTITY_NUMBERS};
use napi::bindgen_prelude::*;
use napi::threadsafe_function::UnknownReturnValue;
use napi_derive::napi;

use crate::error::{refusal, Code};
use crate::forward::{self, Sink, MAX_QUEUE};
use crate::hot::{self, HOT_NUMBERS};

#[napi(object, object_to_js = false)]
pub struct NativeHostOptions {
    /// Shard threads (default: the CPU count, at most 4).
    pub shards: Option<u32>,
    /// Bots publish their state for the sync reads (default true; off, every read is null).
    pub snapshots: Option<bool>,
    /// Gap between joins to one address, ms (default 500; acacia-host's `HostConfig::join_spacing`).
    pub join_spacing_ms: Option<u32>,
    /// Up to this much either way on each gap, ms (default 250; `HostConfig::join_jitter`).
    pub join_jitter_ms: Option<u32>,
    /// Largest decompressed batch a bot accepts (default 128 MiB; `HostConfig::max_batch_bytes`).
    pub max_batch_bytes: Option<u32>,
    #[napi(ts_type = "(batch: string | null) => void")]
    pub on_records: Function<'static, Option<String>, UnknownReturnValue>,
}

#[napi(object, object_from_js = false)]
pub struct EntityDelta {
    pub version: i64,
    pub full: bool,
    /// `{removed?, added?}` JSON (acacia-host's `EntityDelta::members_json`); null when no entity came or went.
    pub members: Option<String>,
    /// The array to use from now on, when the one passed in was too small or the delta is full.
    pub numbers: Option<Float64Array>,
}

/// Slots an entity numbers array starts with.
const MIN_SLOTS: usize = 16;

/// One acacia-host and its record stream (contract: the crate docs).
#[napi]
pub struct NativeHost {
    host: Arc<Host>,
    sink: Arc<Sink>,
    keep_alive: bool,
    closing: bool,
}

#[napi]
impl NativeHost {
    #[napi(constructor)]
    pub fn new(options: NativeHostOptions) -> Result<Self> {
        let sink = options
            .on_records
            .build_threadsafe_function::<Option<String>>()
            .callee_handled::<false>()
            .weak::<true>()
            .max_queue_size::<MAX_QUEUE>()
            .build()?;
        let sink = Arc::new(sink);
        let mut config = HostConfig::default();
        if let Some(shards) = options.shards {
            config.shards = shards as usize;
        }
        config.snapshots = options.snapshots.unwrap_or(config.snapshots);
        let ms = |v: u32| Duration::from_millis(v.into());
        config.join_spacing = options.join_spacing_ms.map_or(config.join_spacing, ms);
        config.join_jitter = options.join_jitter_ms.map_or(config.join_jitter, ms);
        config.max_batch_bytes = options.max_batch_bytes.map_or(config.max_batch_bytes, |b| b as usize);
        #[cfg(feature = "test-server")]
        if let Ok(fake) = std::env::var("ACACIA_FAKE_SIGN_IN") {
            let ms = fake.parse().ok().filter(|&ms: &u64| ms > 1).unwrap_or(300);
            config.sign_in = Some(Arc::new(acacia_host::FakeSignIn::new(Duration::from_millis(ms))));
        }
        let (host, outbound) = Host::start(config).map_err(|e| Error::from_reason(format!("host threads: {e}")))?;
        let host = Arc::new(host);
        forward::spawn(outbound, sink.clone()).map_err(|e| Error::from_reason(format!("forwarder thread: {e}")))?;
        Ok(Self { host, sink, keep_alive: false, closing: false })
    }

    /// Enqueues one command (JSON); throws with the refusal's `code` if the host will not take it.
    #[napi]
    pub fn call(&self, json: String) -> Result<(), Code> {
        self.host.call(&json).map_err(refusal)
    }

    /// Starts shutting the host down; the stream ends with `onRecords(null)`. Idempotent.
    #[napi(catch_unwind)]
    pub fn close(&mut self) -> Result<()> {
        if std::mem::replace(&mut self.closing, true) {
            return Ok(());
        }
        let host = self.host.clone();
        std::thread::Builder::new()
            .name("acacia-node-close".into())
            .spawn(move || host.shutdown_blocking())
            .map(drop)
            .map_err(|e| Error::from_reason(format!("shutdown thread: {e}")))
    }

    /// Whether the record stream holds Node's event loop open.
    #[napi(catch_unwind)]
    pub fn set_keep_alive(&mut self, env: Env, on: bool) -> Result<()> {
        if on == self.keep_alive {
            return Ok(());
        }
        // Raw calls: napi's refer/unref need `&mut` on a function the forwarder shares.
        let status = unsafe {
            match on {
                true => napi::sys::napi_ref_threadsafe_function(env.raw(), self.sink.raw()),
                false => napi::sys::napi_unref_threadsafe_function(env.raw(), self.sink.raw()),
            }
        };
        if status != napi::sys::Status::napi_ok {
            return Err(Error::from_reason(format!("keep-alive {on}: napi status {status}")));
        }
        self.keep_alive = on;
        Ok(())
    }

    /// Writes the bot's `HotSnapshot` as of its last tick into `into` (at least 26 slots, order: crate::hot); false
    /// for an unknown bot or before its first tick.
    #[napi(catch_unwind)]
    pub fn hot_numbers(&self, bot: String, mut into: Float64ArraySlice) -> Result<bool> {
        let Some(hot) = self.host.hot(&BotId(bot)).filter(|h| h.tick != 0) else { return Ok(false) };
        // SAFETY: JS is blocked in this call, so nothing else touches the array.
        let into = unsafe { into.as_mut() };
        let target = into.get_mut(..HOT_NUMBERS).ok_or_else(|| Error::from_reason(format!("hotNumbers needs {HOT_NUMBERS} slots")))?;
        target.copy_from_slice(&hot::numbers(&hot));
        Ok(true)
    }

    /// The version of the bot's `ColdSnapshot`, or null for an unknown bot.
    #[napi(catch_unwind)]
    pub fn cold_version(&self, bot: String) -> Option<i64> {
        self.host.cold_version(&BotId(bot)).map(|v| v as i64)
    }

    /// The bot's `ColdSnapshot` (JSON), or null for an unknown bot.
    #[napi(catch_unwind)]
    pub fn snapshot_cold(&self, bot: String) -> Option<String> {
        serde_json::to_string(&self.host.cold(&BotId(bot))?).ok()
    }

    /// The version of the bot's `EntityList`, or null for an unknown bot.
    #[napi(catch_unwind)]
    pub fn entities_version(&self, bot: String) -> Option<i64> {
        self.host.entities_version(&BotId(bot)).map(|v| v as i64)
    }

    /// The bot's `EntityList` (JSON), or null for an unknown bot.
    #[napi(catch_unwind)]
    pub fn snapshot_entities(&self, bot: String) -> Option<String> {
        serde_json::to_string(&self.host.entities(&BotId(bot))?).ok()
    }

    /// The bot's entity list as changes since `since` (acacia-host's `EntityDelta`), or null for an unknown bot.
    /// The numbers of added and moved entities are written into `numbers` by slot; a new array comes back when
    /// that one is too small (holding the old one's numbers too) or the delta is full (holding only the new ones).
    #[napi(catch_unwind)]
    pub fn entity_delta(&self, bot: String, since: Option<i64>, mut numbers: Float64ArraySlice) -> Option<EntityDelta> {
        let delta = self.host.entity_delta(&BotId(bot), since.map(|v| v as u64))?;
        // SAFETY: JS is blocked in this call, so nothing else touches the array.
        let into = unsafe { numbers.as_mut() };
        let slots = delta.slots() as usize;
        let mut grown = (delta.full() || into.len() < slots * ENTITY_NUMBERS).then(|| {
            let mut grown = vec![f64::NAN; slots.max(MIN_SLOTS).next_power_of_two() * ENTITY_NUMBERS];
            if !delta.full() {
                grown[..into.len()].copy_from_slice(into);
            }
            grown
        });
        let target = grown.as_deref_mut().unwrap_or(into);
        for (slot, row) in delta.numbers() {
            let at = slot as usize * ENTITY_NUMBERS;
            target[at..at + ENTITY_NUMBERS].copy_from_slice(&row);
        }
        let (version, full, members) = (delta.version() as i64, delta.full(), delta.members_json());
        Some(EntityDelta { version, full, members, numbers: grown.map(Float64Array::new) })
    }

    /// The server's `ItemTable` (JSON), or null for an unknown bot; empty until the table arrives at login.
    #[napi(catch_unwind)]
    pub fn item_table(&self, bot: String) -> Option<String> {
        serde_json::to_string(&self.host.item_table(&BotId(bot))?).ok()
    }

    /// `bot.findBlocks` now: `FindBlocksParams` JSON in, `FindBlocksResult` JSON out; throws with the error's `code`.
    #[napi]
    pub fn find_blocks(&self, bot: String, params: String) -> Result<String, Code> {
        json_read(&params, |p: FindBlocksParams| self.host.find_blocks(&BotId(bot), &p))
    }

    /// `bot.blockAtCursor` now: `BlockAtCursorParams` JSON in, `BlockAtCursorResult` JSON out; throws as `find_blocks`.
    #[napi]
    pub fn block_at_cursor(&self, bot: String, params: String) -> Result<String, Code> {
        json_read(&params, |p: BlockAtCursorParams| self.host.block_at_cursor(&BotId(bot), &p))
    }

    /// The `Block` at a cell of the bot's dimension (JSON), or null when unknown (see acacia-host's `block_at`).
    #[napi(catch_unwind)]
    pub fn block_at(&self, bot: String, x: i32, y: i32, z: i32) -> Option<String> {
        let block = self.host.block_at(&BotId(bot), BlockPos { x, y, z })?;
        serde_json::to_string(&block).ok()
    }
}

/// A sync read with JSON params and result; a parse failure is `BAD_PARAMS`.
fn json_read<P: DeserializeOwned, R: Serialize>(params: &str, read: impl FnOnce(P) -> std::result::Result<R, ApiError>) -> Result<String, Code> {
    let params = serde_json::from_str(params).map_err(|e| refusal(ApiError::new(ErrorCode::BadParams, e.to_string())))?;
    let found = read(params).map_err(refusal)?;
    serde_json::to_string(&found).map_err(|e| refusal(ApiError::new(ErrorCode::Internal, e.to_string())))
}
