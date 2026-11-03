use std::collections::HashMap;
use std::sync::Arc;

use acacia_api::{ApiError, BlockPos, BotId, ErrorCode, FindBlocksParams};
use acacia_host::{Host, HostConfig};
use napi::bindgen_prelude::*;
use napi::threadsafe_function::UnknownReturnValue;
use napi_derive::napi;

use crate::error::{refusal, Code};
use crate::forward::{self, Sink, MAX_QUEUE};

#[napi(object, object_to_js = false)]
pub struct NativeHostOptions {
    /// Shard threads (default: the CPU count, at most 4).
    pub shards: Option<u32>,
    #[napi(ts_type = "(batch: string | null) => void")]
    pub on_records: Function<'static, Option<String>, UnknownReturnValue>,
}

#[napi(object, object_from_js = false)]
pub struct EntityFrame {
    pub version: i64,
    pub members_version: i64,
    /// `[id, uniqueId, kind, username | null][]` JSON, in `numbers`' order.
    pub members: Option<String>,
    /// Per member: feet x, y, z, velocity x, y, z, yaw, pitch, health (NaN when unknown).
    pub numbers: Option<Float64Array>,
}

/// One acacia-host and its record stream (contract: the crate docs).
#[napi]
pub struct NativeHost {
    host: Arc<Host>,
    sink: Arc<Sink>,
    keep_alive: bool,
    closing: bool,
    /// Each bot's last hot snapshot JSON and the tick it is from.
    hot: HashMap<String, (u64, String)>,
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
        let (host, outbound) = Host::start(config).map_err(|e| Error::from_reason(format!("host threads: {e}")))?;
        let host = Arc::new(host);
        forward::spawn(outbound, sink.clone()).map_err(|e| Error::from_reason(format!("forwarder thread: {e}")))?;
        Ok(Self { host, sink, keep_alive: false, closing: false, hot: HashMap::new() })
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

    /// The bot's `HotSnapshot` as of its last tick (JSON), or null for an unknown bot or before its first tick.
    #[napi(catch_unwind)]
    pub fn snapshot_hot(&mut self, bot: String) -> Option<String> {
        let Some(hot) = self.host.hot(&BotId(bot.clone())).filter(|h| h.tick != 0) else {
            self.hot.remove(&bot);
            return None;
        };
        if let Some((_, json)) = self.hot.get(&bot).filter(|(tick, _)| *tick == hot.tick) {
            return Some(json.clone());
        }
        let json = serde_json::to_string(&hot).ok()?;
        self.hot.insert(bot, (hot.tick, json.clone()));
        Some(json)
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

    /// The bot's entity list split for per-tick reads (acacia-host's `EntityFrame`), or null for an unknown bot.
    /// While `known` is still the members' version, `members` and `numbers` are null and the numbers are written
    /// into `into` (the array that came with those members).
    #[napi(catch_unwind)]
    pub fn entity_frame(&self, bot: String, known: Option<i64>, mut into: Float64ArraySlice) -> Option<EntityFrame> {
        let frame = self.host.entity_frame(&BotId(bot), known.map(|v| v as u64))?;
        // SAFETY: JS is blocked in this call, so nothing else touches the array.
        let into = unsafe { into.as_mut() };
        let numbers = match frame.members.is_none() && frame.numbers.len() == into.len() {
            true => {
                into.copy_from_slice(&frame.numbers);
                None
            }
            false => Some(Float64Array::new(frame.numbers)),
        };
        let members = frame.members.map(|m| {
            let rows: Vec<_> = m.iter().map(|e| (e.id, e.unique_id, &e.kind, &e.username)).collect();
            serde_json::to_string(&rows).expect("ids and strings serialize")
        });
        Some(EntityFrame { version: frame.version as i64, members_version: frame.members_version as i64, members, numbers })
    }

    /// The server's `ItemTable` (JSON), or null for an unknown bot; empty until the table arrives at login.
    #[napi(catch_unwind)]
    pub fn item_table(&self, bot: String) -> Option<String> {
        serde_json::to_string(&self.host.item_table(&BotId(bot))?).ok()
    }

    /// `bot.findBlocks` now: `FindBlocksParams` JSON in, `FindBlocksResult` JSON out; throws with the error's `code`.
    #[napi]
    pub fn find_blocks(&self, bot: String, params: String) -> Result<String, Code> {
        let params: FindBlocksParams =
            serde_json::from_str(&params).map_err(|e| refusal(ApiError::new(ErrorCode::BadParams, e.to_string())))?;
        let found = self.host.find_blocks(&BotId(bot), &params).map_err(refusal)?;
        serde_json::to_string(&found).map_err(|e| refusal(ApiError::new(ErrorCode::Internal, e.to_string())))
    }

    /// The `Block` at a cell of the bot's dimension (JSON), or null when unknown (see acacia-host's `block_at`).
    #[napi(catch_unwind)]
    pub fn block_at(&self, bot: String, x: i32, y: i32, z: i32) -> Option<String> {
        let block = self.host.block_at(&BotId(bot), BlockPos { x, y, z })?;
        serde_json::to_string(&block).ok()
    }
}
