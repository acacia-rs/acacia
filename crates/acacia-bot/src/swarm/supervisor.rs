use std::future::Future;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use acacia_client::{ClientBuilder, DisconnectReason, Socks5Proxy};
use tokio::sync::{broadcast, watch};
use tokio::time::{sleep_until, Instant};

use super::events::{BotStatus, SwarmEvent};
use super::join_queue::{noise, JoinQueue};
use super::keeper::Keeper;
use super::lease::AccountLease;
use super::login::{client_builder, Failure, NodeAuth};
use super::policy::{Ended, Next, Policy, Retry};
use super::registry::Registry;
use super::spec::{BotId, BotSpec, Login, Target};
use crate::world::SharedWorlds;
use crate::{Bot, BotConfig};

pub(crate) type ConfigFn<S> = dyn Fn(&BotSpec<S>) -> BotConfig + Send + Sync;
pub(crate) type ClientFn<S> = dyn Fn(&BotSpec<S>, ClientBuilder) -> ClientBuilder + Send + Sync;

/// Shared by every supervisor of one swarm.
pub(crate) struct Ctx<S> {
    pub policy: Policy,
    pub joins: JoinQueue,
    pub auth: Option<NodeAuth>,
    pub leases: Arc<dyn AccountLease>,
    pub node_id: String,
    pub lease_ttl: Duration,
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
pub(crate) async fn supervise<S, F>(ctx: Arc<Ctx<S>>, task: Arc<F>, spec: BotSpec<S>, cancel: watch::Receiver<bool>)
where
    F: AsyncFn(&mut Bot, &mut S),
{
    let id = spec.id.clone();
    let keeper = Keeper::new(ctx.leases.clone(), format!("{}/{id}", ctx.node_id), ctx.lease_ttl);
    let mut run = Run { ctx: &ctx, task: &*task, spec, cancel, keeper, retry: Retry::default() };
    let exit = run.until_exit().await;
    run.keeper.release().await;
    match exit {
        Exit::Removed => ctx.removed(&id),
        Exit::Failed(error) => ctx.fail(&id, error),
    }
}

enum Exit {
    Removed,
    Failed(String),
}

enum Interrupt {
    Cancelled,
    LeaseLost,
}

struct Run<'a, S, F> {
    ctx: &'a Ctx<S>,
    task: &'a F,
    spec: BotSpec<S>,
    cancel: watch::Receiver<bool>,
    keeper: Keeper,
    retry: Retry,
}

impl<S, F: AsyncFn(&mut Bot, &mut S)> Run<'_, S, F> {
    async fn until_exit(&mut self) -> Exit {
        let proxy = match self.spec.proxy.as_deref().map(Socks5Proxy::parse).transpose() {
            Ok(p) => p,
            Err(e) => return Exit::Failed(format!("bad proxy: {e}")),
        };
        let id = self.spec.id.clone();
        let mut follow = None;
        let mut attempt = 0;
        loop {
            attempt += 1;
            let target = follow.take().unwrap_or_else(|| self.spec.target.clone());
            self.ctx.registry.set(&id, BotStatus::Waiting);
            let slot = self.ctx.joins.reserve(&target.to_string());
            if !self.idle_until(slot).await || self.ctx.draining.load(Ordering::Relaxed) {
                return Exit::Removed;
            }
            let (next, waiting) = match self.join(&target, attempt, proxy.as_ref()).await {
                Ok(outcome) => outcome,
                Err(exit) => return exit,
            };
            match next {
                Next::After(delay) => {
                    self.ctx.registry.set(&id, waiting);
                    self.ctx.emit(SwarmEvent::Reconnecting { id: id.clone(), after_ms: delay.as_millis() as u64 });
                    if !self.idle_until(Instant::now() + delay).await {
                        return Exit::Removed;
                    }
                }
                Next::Follow(address) => follow = Some(Target::Server { address }),
                Next::Stop(error) => return Exit::Failed(error),
            }
        }
    }

    /// One lease check, join and session; returns what to do next and the status meanwhile.
    async fn join(&mut self, target: &Target, attempt: u32, proxy: Option<&Socks5Proxy>) -> Result<(Next, BotStatus), Exit> {
        let id = self.spec.id.clone();
        if let Login::Online { account } = &self.spec.login {
            let acquired = or_cancel(self.keeper.acquire(account), &mut self.cancel).await.ok_or(Exit::Removed)?;
            if let Err(why) = acquired {
                return Ok(self.lease_unavailable(why));
            }
        }
        self.ctx.registry.set(&id, BotStatus::Connecting);
        self.ctx.emit(SwarmEvent::Joining { id: id.clone(), target: target.clone(), attempt });
        let mut bot = match guard(self.ctx.connect(&self.spec, target, proxy), &mut self.cancel, &mut self.keeper).await {
            Err(Interrupt::Cancelled) => return Err(Exit::Removed),
            Err(Interrupt::LeaseLost) => return Ok(self.lease_unavailable("lease lost while joining".into())),
            Ok(Err(failure)) => return Ok((self.ctx.policy.next(&id, &mut self.retry, failure.ended(), noise()), BotStatus::Backoff)),
            Ok(Ok(bot)) => bot,
        };
        self.ctx.registry.set(&id, BotStatus::Online);
        self.ctx.emit(SwarmEvent::Spawned { id: id.clone(), display_name: bot.client().display_name().to_owned() });
        let since = Instant::now();
        let reason = match guard((self.task)(&mut bot, &mut self.spec.state), &mut self.cancel, &mut self.keeper).await {
            Ok(()) => bot.disconnect_reason().cloned().filter(|r| *r != DisconnectReason::LocalClose),
            Err(Interrupt::Cancelled) => None,
            Err(Interrupt::LeaseLost) => {
                // Leave at once: the account's new holder may already be joining.
                bot.disconnect().await;
                self.ctx.emit(SwarmEvent::Disconnected { id: id.clone(), reason: "lease lost".into() });
                return Ok(self.lease_unavailable("lease lost".into()));
            }
        };
        let Some(reason) = reason else {
            bot.disconnect().await;
            return Err(Exit::Removed);
        };
        self.ctx.emit(SwarmEvent::Disconnected { id: id.clone(), reason: format!("{reason:?}") });
        let ended = Ended::Session { reason: &reason, uptime: since.elapsed() };
        Ok((self.ctx.policy.next(&id, &mut self.retry, ended, noise()), BotStatus::Backoff))
    }

    fn lease_unavailable(&mut self, why: String) -> (Next, BotStatus) {
        (self.ctx.policy.next(&self.spec.id, &mut self.retry, Ended::Lease(why), noise()), BotStatus::AccountBusy)
    }

    /// Waits, renewing any held lease; false once cancelled. A lease lost meanwhile is re-acquired
    /// before the next join.
    async fn idle_until(&mut self, until: Instant) -> bool {
        loop {
            match guard(sleep_until(until), &mut self.cancel, &mut self.keeper).await {
                Ok(()) => return true,
                Err(Interrupt::Cancelled) => return false,
                Err(Interrupt::LeaseLost) => {}
            }
        }
    }
}

async fn guard<T>(fut: impl Future<Output = T>, cancel: &mut watch::Receiver<bool>, keeper: &mut Keeper) -> Result<T, Interrupt> {
    tokio::select! {
        v = fut => Ok(v),
        _ = cancel.wait_for(|c| *c) => Err(Interrupt::Cancelled),
        () = keeper.lost() => Err(Interrupt::LeaseLost),
    }
}

/// `None` once the bot is cancelled (or the swarm dropped its handle).
async fn or_cancel<T>(fut: impl Future<Output = T>, cancel: &mut watch::Receiver<bool>) -> Option<T> {
    tokio::select! {
        v = fut => Some(v),
        _ = cancel.wait_for(|c| *c) => None,
    }
}
