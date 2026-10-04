//! The bot's thread: connects, keeps the bot polled, and reports world changes to the window.

use std::sync::Arc;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::time::{Duration, Instant};

use acacia_bot::client::{Client, ClientBuilder, PacketFilter};
use acacia_bot::proto::Packet;
use acacia_bot::proto::packets::BiomeDefinitionList;
use acacia_bot::{Bot, BotConfig, BotEvent};
use acacia_render::assets::Pack;
use acacia_render::biome::{BiomeColors, BiomeDef};
use acacia_render::assets::flipbook::Atlas;
use acacia_bot::state::Trackers;
use acacia_render::block_models::BlockDataMap;
use acacia_render::blocks::BlockTable;
use acacia_render::entity::EntityModels;
use acacia_world::World;

use crate::entities::{Feed, SNAPSHOT_SECS, Tracked};
use glam::DVec3;
use tokio::sync::oneshot;

/// Reports between looks at the block entities for changed model data.
const BLOCK_DATA_EVERY: u32 = 10;

pub struct Options {
    pub server: String,
    pub name: String,
    pub radius: i32,
}

pub enum NetEvent {
    /// A new world (join or dimension change) with render data built from its registry.
    World { world: Arc<World>, table: Arc<BlockTable>, textures: Atlas },
    /// Biome colours from the server's `BiomeDefinitionList`.
    Biomes(Arc<BiomeColors>),
    /// The bot's eye position.
    Player(DVec3),
    /// Sent once, before any [`NetEvent::Entities`].
    EntityModels(Arc<EntityModels>),
    Entities(Vec<Tracked>),
    /// The world time in ticks, when the server sends a new one.
    Time(i32),
    /// What the block entities add to block models, when it changes.
    BlockData(Arc<BlockDataMap>),
    Status(String),
    /// The bot thread stopped: kicked, disconnected, or failed to join. Last event sent.
    Ended(String),
}

/// The running bot thread. [`Net::shutdown`] disconnects cleanly: a bot that just vanishes keeps its
/// session alive server-side until timeout, and BDS kicks a rejoin under the same name
/// (`ServerIdConflict`).
pub struct Net {
    pub events: Receiver<NetEvent>,
    quit: Option<oneshot::Sender<()>>,
}

impl Net {
    /// Asks the bot to disconnect and waits (up to 3 s) for its thread to end, which drops the
    /// event sender.
    pub fn shutdown(&mut self) {
        let Some(quit) = self.quit.take() else { return };
        let _ = quit.send(());
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            match self.events.recv_timeout(left) {
                Ok(NetEvent::Ended(reason)) => tracing::info!("session ended: {reason}"),
                Ok(_) => {}
                Err(RecvTimeoutError::Disconnected) => break,
                Err(RecvTimeoutError::Timeout) => {
                    tracing::warn!("bot did not disconnect within 3 s; the server may hold the session (ServerIdConflict)");
                    break;
                }
            }
        }
    }
}

pub fn spawn(options: Options, pack: Pack) -> Net {
    let (tx, events) = channel();
    let (quit, quit_rx) = oneshot::channel();
    std::thread::Builder::new()
        .name("bot".into())
        .spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().expect("tokio runtime");
            let reason = rt.block_on(run(options, pack, &tx, quit_rx)).unwrap_or_else(|e| format!("error: {e}"));
            let _ = tx.send(NetEvent::Ended(reason));
        })
        .expect("spawn bot thread");
    Net { events, quit: Some(quit) }
}

/// Returns why the session ended.
async fn run(options: Options, pack: Pack, tx: &Sender<NetEvent>, mut quit: oneshot::Receiver<()>) -> Result<String, Box<dyn std::error::Error>> {
    let send = |e| tx.send(e).map_err(|_| "window closed");
    send(NetEvent::Status(format!("connecting to {}", options.server)))?;
    let builder = login(Client::builder(&options.server).chunk_radius(options.radius), &options.name).await?;
    let subscribe = PacketFilter::none().with(BiomeDefinitionList::ID);
    let trackers = Trackers { entities: true, skins: true, ..Trackers::default() };
    let config = BotConfig { physics: true, auto_respawn: true, subscribe, trackers, ..BotConfig::default() };
    let models = Arc::new(EntityModels::load(&pack));
    send(NetEvent::EntityModels(models.clone()))?;
    let mut feed = Feed::new(models);
    let mut bot = tokio::select! {
        bot = Bot::connect(builder, config) => bot?,
        _ = &mut quit => return Ok("quit".into()),
    };
    send(NetEvent::Status(format!("joined as {}", bot.client().display_name())))?;
    // `ACACIA_COMMANDS="summon cow;time set day"`: setup for unattended shots (needs an operator).
    for command in std::env::var("ACACIA_COMMANDS").iter().flat_map(|s| s.split(';')) {
        bot.client().command(command.trim());
    }

    let mut current: Option<Arc<World>> = None;
    let mut biome_logged = false;
    let mut time = None;
    let mut block_data = Arc::new(BlockDataMap::new());
    let mut reports = 0u32;
    // `next` only returns for caller-facing events, which a viewer barely subscribes to; the
    // timer reports world and position changes in between (`next` is cancel-safe).
    let mut report = tokio::time::interval(Duration::from_secs_f32(SNAPSHOT_SECS));
    loop {
        tokio::select! {
            event = bot.next() => match event {
                Some(BotEvent::Disconnected(reason)) => return Ok(format!("disconnected: {reason:?}")),
                None => return Ok("bot stopped".into()),
                Some(BotEvent::Packet(p)) if p.id == BiomeDefinitionList::ID => {
                    let defs = biome_defs(&p.decode()?);
                    tracing::info!(count = defs.len(), "biome definitions");
                    let colors = BiomeColors::build(&defs, &pack);
                    send(NetEvent::Biomes(Arc::new(colors)))?;
                    continue;
                }
                Some(_) => continue,
            },
            _ = report.tick() => {}
            _ = &mut quit => {
                bot.disconnect().await;
                return Ok("quit".into());
            }
        }
        let world = bot.world().and_then(|w| w.view()).map(|v| v.world().clone());
        if let Some(world) = world.filter(|w| current.as_ref().is_none_or(|c| !Arc::ptr_eq(c, w))) {
            let (table, textures, built) = BlockTable::build(world.registry(), &pack);
            tracing::info!(
                textures = textures.layers.len(),
                animated = textures.animations.len(),
                untextured = built.blocks_without_textures.len(),
                missing_images = built.missing_images.len(),
                "block table"
            );
            tracing::debug!(untextured = ?built.blocks_without_textures, missing = ?built.missing_images);
            current = Some(world.clone());
            send(NetEvent::World { world, table: Arc::new(table), textures })?;
        }
        if let Some(world) = &current {
            let p = bot.state().player.eye_position();
            send(NetEvent::Player(DVec3::new(p.x.into(), p.y.into(), p.z.into())))?;
            send(NetEvent::Entities(feed.snapshot(&bot)))?;
            let now = bot.state().environment.time;
            if time.replace(now) != Some(now) {
                tracing::debug!(time = now, "time of day");
                send(NetEvent::Time(now))?;
            }
            reports += 1;
            if reports % BLOCK_DATA_EVERY == 0 {
                let data = crate::block_data::snapshot(&bot.state().block_entities);
                if data != *block_data {
                    block_data = Arc::new(data);
                    send(NetEvent::BlockData(block_data.clone()))?;
                }
            }
            if !biome_logged {
                let (x, y, z) = (p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32);
                if let Some(id) = world.get(x >> 4, z >> 4).and_then(|c| c.read().biome(x, y, z)) {
                    tracing::debug!(id, x, y, z, "biome under the bot");
                    biome_logged = true;
                }
            }
        }
    }
}

fn biome_defs(list: &BiomeDefinitionList) -> Vec<BiomeDef> {
    list.biome_definitions
        .iter()
        .filter_map(|d| {
            let name = list.string_list.get(usize::try_from(d.name_index).ok()?)?;
            let name = name.strip_prefix("minecraft:").unwrap_or(name).to_owned();
            Some(BiomeDef { id: d.biome_id, name, temperature: d.temperature, downfall: d.downfall })
        })
        .collect()
}

async fn login(builder: ClientBuilder, name: &str) -> Result<ClientBuilder, Box<dyn std::error::Error>> {
    let Some(account) = name.strip_prefix('@') else { return Ok(builder.offline(name)) };
    use acacia_bot::client::auth::{Account, AuthClient, AuthConfig, FileTokenCache};
    let account = Account::new(Arc::new(AuthClient::new(AuthConfig::default())?), Arc::new(FileTokenCache::new(".tokens")?), account);
    let (key, credentials) = account.login_credentials().await?;
    Ok(builder.online(credentials, key))
}
