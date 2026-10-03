use std::future::Future;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use acacia_client::{ClientBuilder, DisconnectReason, Socks5Proxy};
use tokio::sync::{broadcast, watch};
use tokio::time::{sleep, sleep_until, Instant};

use super::events::{BotStatus, SwarmEvent};
use super::join_queue::{noise, JoinQueue};
use super::login::{client_builder, Failure, NodeAuth};
use super::policy::{Ended, Next, Policy, Retry};
use super::registry::Registry;
use super::spec::{BotId, BotSpec, Target};
use crate::world::SharedWorlds;
use crate::{Bot, BotConfig};

pub(crate) type ConfigFn<S> = dyn Fn(&BotSpec<S>) -> BotConfig + Send + Sync;
pub(crate) type ClientFn<S> = dyn Fn(&BotSpec<S>, ClientBuilder) -> ClientBuilder + Send + Sync;

/// Shared by every supervisor of one swarm.
pub(crate) struct Ctx<S> {
    pub policy: Policy,
    pub joins: JoinQueue,
    pub auth: Option<NodeAuth>,
    pub worlds: SharedWorlds,
    pub bot_config: Arc<ConfigFn<S>>,
    pub client: Arc<ClientFn<S>>,
    pub registry: Registry,
    pub events: broadcast::Sender<SwarmEvent>,
    pub draining: AtomicBool,
}

impl<S> Ctx<S> {
    pub fn emit(&self, event: SwarmEvent) {
        let _ = self.events.send(event);
    }

    fn removed(&self, id: &BotId) {
        self.registry.remove(id);
        self.emit(SwarmEvent::Removed { id: id.clone() });
    }

    pub fn fail(&self, id: &BotId, error: String) {
        tracing::warn!(%id, %error, "bot stopped");
        self.registry.set(id, BotStatus::Failed { error: error.clone() });
        self.emit(SwarmEvent::Failed { id: id.clone(), error });
    }

    async fn connect(&self, spec: &BotSpec<S>, target: &Target, proxy: Option<&Socks5Proxy>) -> Result<Bot, Failure> {
        let builder = client_builder(&spec.login, target, proxy, self.auth.as_ref()).await?;
        let builder = (self.client)(spec, builder);
        let config = BotConfig { shared_worlds: self.worlds.clone(), ..(self.bot_config)(spec) };
        Bot::connect(builder, config).await.map_err(Failure::Connect)
    }
}

/// Runs one bot until it is removed, fails for good, or its task returns while connected.
pub(crate) async fn supervise<S, F>(ctx: Arc<Ctx<S>>, task: Arc<F>, mut spec: BotSpec<S>, mut cancel: watch::Receiver<bool>)
where
    F: AsyncFn(&mut Bot, &mut S),
{
    let id = spec.id.clone();
    let proxy = match spec.proxy.as_deref().map(Socks5Proxy::parse).transpose() {
        Ok(p) => p,
        Err(e) => return ctx.fail(&id, format!("bad proxy: {e}")),
    };
    let mut retry = Retry::default();
    let mut follow = None;
    for attempt in 1.. {
        let target = follow.take().unwrap_or_else(|| spec.target.clone());
        ctx.registry.set(&id, BotStatus::Waiting);
        let slot = ctx.joins.reserve(&target.to_string());
        if or_cancel(sleep_until(slot), &mut cancel).await.is_none() || ctx.draining.load(Ordering::Relaxed) {
            return ctx.removed(&id);
        }
        ctx.registry.set(&id, BotStatus::Connecting);
        ctx.emit(SwarmEvent::Joining { id: id.clone(), target: target.clone(), attempt });
        let Some(connected) = or_cancel(ctx.connect(&spec, &target, proxy.as_ref()), &mut cancel).await else {
            return ctx.removed(&id);
        };
        let next = match connected {
            Err(failure) => ctx.policy.next(&id, &mut retry, failure.ended(), noise()),
            Ok(mut bot) => {
                ctx.registry.set(&id, BotStatus::Online);
                ctx.emit(SwarmEvent::Spawned { id: id.clone(), display_name: bot.client().display_name().to_owned() });
                let since = Instant::now();
                let finished = or_cancel(task(&mut bot, &mut spec.state), &mut cancel).await;
                let reason = bot.disconnect_reason().cloned();
                let Some(reason) = reason.filter(|r| finished.is_some() && *r != DisconnectReason::LocalClose) else {
                    bot.disconnect().await;
                    return ctx.removed(&id);
                };
                ctx.emit(SwarmEvent::Disconnected { id: id.clone(), reason: format!("{reason:?}") });
                let ended = Ended::Session { reason: &reason, uptime: since.elapsed() };
                ctx.policy.next(&id, &mut retry, ended, noise())
            }
        };
        match next {
            Next::After(delay) => {
                ctx.registry.set(&id, BotStatus::Backoff);
                ctx.emit(SwarmEvent::Reconnecting { id: id.clone(), after_ms: delay.as_millis() as u64 });
                if or_cancel(sleep(delay), &mut cancel).await.is_none() {
                    return ctx.removed(&id);
                }
            }
            Next::Follow(address) => follow = Some(Target::Server { address }),
            Next::Stop(error) => return ctx.fail(&id, error),
        }
    }
}

/// `None` once the bot is cancelled (or the swarm dropped its handle).
async fn or_cancel<T>(fut: impl Future<Output = T>, cancel: &mut watch::Receiver<bool>) -> Option<T> {
    tokio::select! {
        v = fut => Some(v),
        _ = cancel.wait_for(|c| *c) => None,
    }
}
