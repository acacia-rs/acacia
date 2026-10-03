//! The bot's thread: connects, keeps the bot polled, and reports world changes to the window.

use std::sync::Arc;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::time::{Duration, Instant};

use acacia_bot::client::{Client, ClientBuilder};
use acacia_bot::{Bot, BotConfig, BotEvent};
use acacia_render::assets::Pack;
use acacia_render::assets::image::Texture;
use acacia_render::blocks::BlockTable;
use acacia_world::World;
use glam::DVec3;
use tokio::sync::oneshot;

pub struct Options {
    pub server: String,
    pub name: String,
    pub radius: i32,
}

pub enum NetEvent {
    /// A new world (join or dimension change) with render data built from its registry.
    World { world: Arc<World>, table: Arc<BlockTable>, textures: Vec<Texture> },
    /// The bot's eye position.
    Player(DVec3),
    Status(String),
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
        while let Some(left) = deadline.checked_duration_since(Instant::now()) {
            if let Err(RecvTimeoutError::Disconnected) = self.events.recv_timeout(left) {
                break;
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
            if let Err(e) = rt.block_on(run(options, pack, &tx, quit_rx)) {
                let _ = tx.send(NetEvent::Status(format!("error: {e}")));
            }
        })
        .expect("spawn bot thread");
    Net { events, quit: Some(quit) }
}

async fn run(options: Options, pack: Pack, tx: &Sender<NetEvent>, mut quit: oneshot::Receiver<()>) -> Result<(), Box<dyn std::error::Error>> {
    let send = |e| tx.send(e).map_err(|_| "window closed");
    send(NetEvent::Status(format!("connecting to {}", options.server)))?;
    let builder = login(Client::builder(&options.server).chunk_radius(options.radius), &options.name).await?;
    let config = BotConfig { physics: true, auto_respawn: true, ..BotConfig::default() };
    let mut bot = tokio::select! {
        bot = Bot::connect(builder, config) => bot?,
        _ = &mut quit => return Ok(()),
    };
    send(NetEvent::Status(format!("joined as {}", bot.client().display_name())))?;

    let mut current: Option<Arc<World>> = None;
    // `next` only returns for caller-facing events, which a viewer barely subscribes to; the
    // timer reports world and position changes in between (`next` is cancel-safe).
    let mut report = tokio::time::interval(std::time::Duration::from_millis(50));
    loop {
        tokio::select! {
            event = bot.next() => match event {
                Some(BotEvent::Disconnected(reason)) => {
                    send(NetEvent::Status(format!("disconnected: {reason:?}")))?;
                    break;
                }
                None => break,
                Some(_) => continue,
            },
            _ = report.tick() => {}
            _ = &mut quit => {
                bot.disconnect().await;
                break;
            }
        }
        let world = bot.world().and_then(|w| w.view()).map(|v| v.world().clone());
        if let Some(world) = world.filter(|w| current.as_ref().is_none_or(|c| !Arc::ptr_eq(c, w))) {
            let (table, textures, built) = BlockTable::build(world.registry(), &pack);
            tracing::info!(
                textures = textures.len(),
                untextured = built.blocks_without_textures.len(),
                missing_images = built.missing_images.len(),
                "block table"
            );
            tracing::debug!(untextured = ?built.blocks_without_textures, missing = ?built.missing_images);
            current = Some(world.clone());
            send(NetEvent::World { world, table: Arc::new(table), textures })?;
        }
        if current.is_some() {
            let p = bot.state().player.eye_position();
            send(NetEvent::Player(DVec3::new(p.x.into(), p.y.into(), p.z.into())))?;
        }
    }
    Ok(())
}

async fn login(builder: ClientBuilder, name: &str) -> Result<ClientBuilder, Box<dyn std::error::Error>> {
    let Some(account) = name.strip_prefix('@') else { return Ok(builder.offline(name)) };
    use acacia_bot::client::auth::{Account, AuthClient, AuthConfig, FileTokenCache};
    let account = Account::new(Arc::new(AuthClient::new(AuthConfig::default())?), Arc::new(FileTokenCache::new(".tokens")?), account);
    let (key, credentials) = account.login_credentials().await?;
    Ok(builder.online(credentials, key))
}
