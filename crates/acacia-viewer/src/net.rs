//! The bot's thread: connects, keeps the bot polled, and reports world changes to the window.

mod packets;
mod setup;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::time::{Duration, Instant};

use acacia_bot::client::{Client, ClientBuilder, PacketFilter};
use acacia_bot::{Bot, BotConfig, BotEvent, Events};
use acacia_render::biome::BiomeColors;
use acacia_bot::state::Trackers;
use acacia_render::block_models::BlockDataMap;
use acacia_render::entity::EntityModels;
use acacia_world::World;

use crate::entities::{Feed, SNAPSHOT_SECS, Tracked};
use glam::DVec3;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
use tokio::sync::oneshot;

use crate::audio;
use crate::control::{self, Command};
use crate::me::{self, Me};

/// Reports between looks at the block entities for changed model data.
const BLOCK_DATA_EVERY: u32 = 10;

pub struct Options {
    pub server: String,
    pub name: String,
    pub radius: i32,
}

pub enum NetEvent {
    /// A new world (join or dimension change).
    World(Arc<World>),
    /// Biome colours from the server's `BiomeDefinitionList`.
    Biomes(Arc<BiomeColors>),
    /// The bot's eye position.
    Player(DVec3),
    /// The player after each tick.
    Me(Me),
    Sound(audio::Cue),
    /// A block broke there; `block` is its runtime id, for the chips.
    Broken { pos: glam::IVec3, block: u32 },
    Particles(crate::particles::Spawn),
    /// A title, subtitle or action bar text (or a clear).
    Title(acacia_bot::events::Title),
    /// The player list's names, sorted, when they changed.
    Players(Vec<String>),
    /// The player's slots, when they changed.
    Inventory(control::Inventory),
    /// The creative inventory's items, once the server sent them.
    Creative(Vec<control::CreativeEntry>),
    /// The server form now open, when that changed (`None`: it closed).
    Form(Option<acacia_bot::forms::Form>),
    /// The server opened a sign's editor, on this text (and whether the sign hangs); `None`: it closed.
    SignEditor(Option<(String, bool)>),
    /// The scoreboard sidebar, when it changed (`None`: none shown).
    Sidebar(Option<acacia_ui::sidebar::Sidebar>),
    /// A chat line, with `§` codes; `message` may be a `%key` that `params` fill.
    /// `rawtext` is the message's JSON when it came as one (`acacia_bot::events` `ChatMessage`).
    Chat { sender: Option<String>, message: String, params: Vec<String>, rawtext: Option<String> },
    /// Sent once, before any [`NetEvent::Entities`].
    EntityModels(Arc<EntityModels>),
    Entities(Vec<Tracked>),
    /// The world time in ticks, when the server sends a new one.
    Time(i32),
    /// What the block entities add to block models, when it changes.
    BlockData(Arc<BlockDataMap>),
    /// The written signs, when one changes.
    Signs(Arc<crate::sign_text::Texts>),
    Status(String),
    /// The bot thread stopped: kicked, disconnected, or failed to join. Last event sent.
    Ended(String),
}

/// The running bot thread. [`Net::shutdown`] disconnects cleanly: a bot that just vanishes keeps its
/// session alive server-side until timeout, and BDS kicks a rejoin under the same name
/// (`ServerIdConflict`).
pub struct Net {
    pub events: Receiver<NetEvent>,
    pub commands: UnboundedSender<Command>,
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

/// `files` is the look pack's ([`acacia_render::LookPack::files`]).
pub fn spawn(options: Options, files: PathBuf) -> Net {
    let (tx, events) = channel();
    let (quit, quit_rx) = oneshot::channel();
    let (commands, commands_rx) = unbounded_channel();
    std::thread::Builder::new()
        .name("bot".into())
        .spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().expect("tokio runtime");
            let reason = rt.block_on(run(options, &files, &tx, quit_rx, commands_rx)).unwrap_or_else(|e| format!("error: {e}"));
            let _ = tx.send(NetEvent::Ended(reason));
        })
        .expect("spawn bot thread");
    Net { events, commands, quit: Some(quit) }
}

/// Returns why the session ended.
async fn run(
    options: Options,
    files: &Path,
    tx: &Sender<NetEvent>,
    mut quit: oneshot::Receiver<()>,
    mut commands: UnboundedReceiver<Command>,
) -> Result<String, Box<dyn std::error::Error>> {
    let send = |e| tx.send(e).map_err(|_| "window closed");
    send(NetEvent::Status(format!("connecting to {}", options.server)))?;
    let builder = login(Client::builder(&options.server).chunk_radius(options.radius), &options.name).await?;
    let subscribe = packets::forwarded().fold(PacketFilter::none(), PacketFilter::with);
    let trackers = Trackers { entities: true, skins: true, ..Trackers::default() };
    let events = Events::TICKS | Events::CHAT | Events::TITLES;
    // The death screen respawns (control::Command::Respawn), as a player does.
    let config = BotConfig { physics: true, auto_respawn: false, subscribe, trackers, events, mouse_input: true, ..BotConfig::default() };
    let models = Arc::new(EntityModels::load(files));
    send(NetEvent::EntityModels(models.clone()))?;
    let mut feed = Feed::new(models);
    let mut bot = tokio::select! {
        bot = Bot::connect(builder, config) => bot?,
        _ = &mut quit => return Ok("quit".into()),
    };
    send(NetEvent::Status(format!("joined as {}", bot.client().display_name())))?;
    let mut inventory = control::Inventory::default();
    // How many creative items were last sent to the window.
    let mut creative = 0;
    // The text in an open anvil's name box, and whether it changed since the result was worked out.
    let (mut name, mut renamed) = (None::<String>, false);
    let mut own_sounds = audio::Own::default();
    let mut ride_log = crate::ride::RideLog::default();
    let mut players: Vec<String> = Vec::new();
    let (mut form, mut sign_open): (Option<u32>, bool) = (None, false);
    let mut sidebar: Option<acacia_ui::sidebar::Sidebar> = None;
    let mut setup = setup::Setup::from_env();

    let mut current: Option<Arc<World>> = None;
    let mut biome_logged = false;
    let mut time = None;
    let (mut block_data, mut signs) = (Arc::new(BlockDataMap::new()), Arc::new(crate::sign_text::Texts::new()));
    let mut reports = 0u32;
    // `next` only returns for caller-facing events, which a viewer barely subscribes to; the
    // timer reports world and position changes in between (`next` is cancel-safe).
    let mut report = tokio::time::interval(Duration::from_secs_f32(SNAPSHOT_SECS));
    loop {
        tokio::select! {
            event = bot.next() => match event {
                Some(BotEvent::Disconnected(reason)) => return Ok(format!("disconnected: {reason:?}")),
                None => return Ok("bot stopped".into()),
                Some(BotEvent::Packet(p)) => {
                    for event in packets::events(&bot, &p, files)? {
                        send(event)?;
                    }
                    continue;
                }
                Some(BotEvent::Chat(m)) => {
                    let sender = m.sender.filter(|s| !s.is_empty());
                    send(NetEvent::Chat { sender, message: m.message, params: m.params, rawtext: m.rawtext })?;
                    continue;
                }
                Some(BotEvent::Title(t)) => {
                    send(NetEvent::Title(t))?;
                    continue;
                }
                Some(BotEvent::Tick) => {
                    setup.tick(&bot);
                    ride_log.tick(&bot);
                    send(NetEvent::Me(me::me(&bot)))?;
                    let own = own_sounds.tick(&bot);
                    for cue in own.sounds {
                        send(NetEvent::Sound(cue))?;
                    }
                    if let Some((pos, block)) = own.broken {
                        send(NetEvent::Broken { pos, block })?;
                    }
                    // `inventory` is compared without what is craftable, which is worked out only on a change.
                    let now = control::inventory(&bot);
                    if now != inventory || std::mem::take(&mut renamed) {
                        if now.station != inventory.station || now.container.is_some() != inventory.container.is_some() {
                            tracing::debug!(open = ?bot.state().containers.open.as_ref().map(|c| (c.window_type, c.slots.len())), station = ?now.station, "screen");
                        }
                        inventory = now.clone();
                        let table = now.workbench.is_some();
                        let (craftable, crafted) = (control::craftable(&bot, table), control::crafted(&bot, name.as_deref()));
                        send(NetEvent::Inventory(control::Inventory { craftable, crafted, ..now }))?;
                    }
                    if creative != bot.state().creative.items().len() {
                        creative = bot.state().creative.items().len();
                        send(NetEvent::Creative(control::creative(&bot)))?;
                    }
                    let names = control::player_names(&bot);
                    if names != players {
                        players = names.clone();
                        send(NetEvent::Players(names))?;
                    }
                    let open = bot.state().forms.latest().map(|f| f.id);
                    if open != form {
                        form = open;
                        send(NetEvent::Form(bot.state().forms.latest().cloned()))?;
                    }
                    let editing = control::open_sign(&bot);
                    if editing.is_some() != sign_open {
                        sign_open = editing.is_some();
                        send(NetEvent::SignEditor(editing))?;
                    }
                    let now = crate::scoreboard::sidebar(&bot);
                    if now != sidebar {
                        sidebar = now.clone();
                        send(NetEvent::Sidebar(now))?;
                    }
                    continue;
                }
                Some(_) => continue,
            },
            Some(command) = commands.recv() => {
                if let control::Command::Name(new) = &command {
                    (name, renamed) = (new.clone(), true);
                }
                control::apply(&mut bot, command).await;
                continue;
            }
            _ = report.tick() => {}
            _ = &mut quit => {
                bot.disconnect().await;
                return Ok("quit".into());
            }
        }
        let world = bot.world().and_then(|w| w.view()).map(|v| v.world().clone());
        if let Some(world) = world.filter(|w| current.as_ref().is_none_or(|c| !Arc::ptr_eq(c, w))) {
            current = Some(world.clone());
            send(NetEvent::World(world))?;
        }
        if let Some(world) = &current {
            let p = bot.state().player.eye_position();
            send(NetEvent::Player(DVec3::new(p.x.into(), p.y.into(), p.z.into())))?;
            send(NetEvent::Entities(feed.snapshot(&bot)))?;
            let now = bot.state().environment.time;
            if time.replace(now) != Some(now) {
                tracing::debug!(time = now, rain = bot.state().environment.rain, "time of day");
                send(NetEvent::Time(now))?;
            }
            reports += 1;
            if reports.is_multiple_of(BLOCK_DATA_EVERY) {
                let data = crate::block_data::snapshot(&bot.state().block_entities);
                if data != *block_data {
                    block_data = Arc::new(data);
                    send(NetEvent::BlockData(block_data.clone()))?;
                }
                let texts = crate::sign_text::snapshot(&bot.state().block_entities);
                if texts != *signs {
                    signs = Arc::new(texts);
                    send(NetEvent::Signs(signs.clone()))?;
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

async fn login(builder: ClientBuilder, name: &str) -> Result<ClientBuilder, Box<dyn std::error::Error>> {
    let Some(account) = name.strip_prefix('@') else { return Ok(builder.offline(name)) };
    use acacia_bot::client::auth::{Account, AuthClient, AuthConfig, FileTokenCache};
    let account = Account::new(Arc::new(AuthClient::new(AuthConfig::default())?), Arc::new(FileTokenCache::new(".tokens")?), account);
    let (key, credentials) = account.login_credentials().await?;
    Ok(builder.online(credentials, key))
}
