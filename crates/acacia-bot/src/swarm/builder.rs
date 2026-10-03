use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use acacia_client::auth::{AuthConfig, TokenCache};
use acacia_client::ClientBuilder;
use tokio::sync::broadcast;

use super::join_queue::{noise, JoinQueue};
use super::lease::{AccountLease, LocalLeases};
use super::login::NodeAuth;
use super::policy::Policy;
use super::registry::Registry;
use super::shard::{Job, OnPanic, Shard};
use super::spec::BotSpec;
use super::supervisor::{supervise, ClientFn, ConfigFn, Ctx};
use super::{Inner, Swarm};
use crate::world::SharedWorlds;
use crate::{Bot, BotConfig};

pub struct SwarmBuilder<S> {
    shards: usize,
    policy: Policy,
    join_delay: Duration,
    join_jitter: Duration,
    auth_config: AuthConfig,
    token_cache: Option<Arc<dyn TokenCache>>,
    leases: Arc<dyn AccountLease>,
    node_id: String,
    lease_ttl: Duration,
    bot_config: Arc<ConfigFn<S>>,
    client: Arc<ClientFn<S>>,
    worlds: SharedWorlds,
    event_capacity: usize,
}

impl<S: Send + 'static> Default for SwarmBuilder<S> {
    fn default() -> Self {
        Self {
            shards: std::thread::available_parallelism().map_or(1, usize::from),
            policy: Policy::default(),
            join_delay: Duration::from_millis(500),
            join_jitter: Duration::from_millis(250),
            auth_config: AuthConfig::default(),
            token_cache: None,
            leases: Arc::new(LocalLeases::new()),
            node_id: format!("node-{:016x}", noise()),
            lease_ttl: Duration::from_secs(30),
            bot_config: Arc::new(|_| BotConfig::default()),
            client: Arc::new(|_, builder| builder),
            worlds: SharedWorlds::new(),
            event_capacity: 1024,
        }
    }
}

impl<S: Send + 'static> SwarmBuilder<S> {
    /// Threads to run bots on (default: one per core).
    pub fn shards(mut self, shards: usize) -> Self {
        self.shards = shards.max(1);
        self
    }

    pub fn policy(mut self, policy: Policy) -> Self {
        self.policy = policy;
        self
    }

    /// Gap between joins to the same target (default 500 ± 250 ms).
    pub fn join_spacing(mut self, delay: Duration, jitter: Duration) -> Self {
        self.join_delay = delay;
        self.join_jitter = jitter;
        self
    }

    /// Where online accounts' tokens live; required for [`super::Login::Online`].
    pub fn token_cache(mut self, cache: Arc<dyn TokenCache>) -> Self {
        self.token_cache = Some(cache);
        self
    }

    /// Where online accounts' leases live (default: this process only). Share one store across
    /// nodes that may run the same accounts.
    pub fn leases(mut self, leases: Arc<dyn AccountLease>) -> Self {
        self.leases = leases;
        self
    }

    /// This node's name in lease holders (`<node>/<bot id>`); default random.
    pub fn node_id(mut self, id: impl Into<String>) -> Self {
        self.node_id = id.into();
        self
    }

    /// How long a lease outlives its holder going silent (default 30 s); renewed every third.
    pub fn lease_ttl(mut self, ttl: Duration) -> Self {
        self.lease_ttl = ttl;
        self
    }

    /// Title and game version for sign-in; the proxy is set per bot from its spec.
    pub fn auth_config(mut self, config: AuthConfig) -> Self {
        self.auth_config = config;
        self
    }

    /// The bot config for each connection; `shared_worlds` is always the swarm's own.
    pub fn bot_config(mut self, f: impl Fn(&BotSpec<S>) -> BotConfig + Send + Sync + 'static) -> Self {
        self.bot_config = Arc::new(f);
        self
    }

    /// Adjusts each connection's client after sign-in (blob cache, transport, chunk radius...).
    pub fn client(mut self, f: impl Fn(&BotSpec<S>, ClientBuilder) -> ClientBuilder + Send + Sync + 'static) -> Self {
        self.client = Arc::new(f);
        self
    }

    /// Share worlds with bots outside this swarm.
    pub fn shared_worlds(mut self, worlds: SharedWorlds) -> Self {
        self.worlds = worlds;
        self
    }

    /// Events buffered per subscriber before a slow one starts missing them.
    pub fn event_capacity(mut self, capacity: usize) -> Self {
        self.event_capacity = capacity.max(1);
        self
    }

    /// Starts the shard threads. `task` drives one connection of one bot; when it returns after a
    /// disconnect the swarm reconnects (per the policy) and calls it again with the same state.
    pub fn start<F>(self, task: F) -> std::io::Result<Swarm<S>>
    where
        F: AsyncFn(&mut Bot, &mut S) + Send + Sync + 'static,
    {
        let ctx = Arc::new(Ctx {
            policy: self.policy,
            joins: JoinQueue::new(self.join_delay, self.join_jitter),
            auth: self.token_cache.map(|cache| NodeAuth::new(self.auth_config, cache)),
            leases: self.leases,
            node_id: self.node_id,
            lease_ttl: self.lease_ttl,
            worlds: self.worlds,
            bot_config: self.bot_config,
            client: self.client,
            registry: Registry::default(),
            events: broadcast::channel(self.event_capacity).0,
            draining: AtomicBool::new(false),
        });
        let on_panic: OnPanic = {
            let ctx = ctx.clone();
            Arc::new(move |id, error| ctx.fail(id, format!("panicked: {error}")))
        };
        let shards = (0..self.shards).map(|i| Shard::start(i, on_panic.clone())).collect::<std::io::Result<Vec<_>>>()?;
        let task = Arc::new(task);
        let spawn_ctx = ctx.clone();
        let spawn = Box::new(move |spec, cancel| -> Job {
            let (ctx, task) = (spawn_ctx.clone(), task.clone());
            Box::new(move || Box::pin(supervise(ctx, task, spec, cancel)) as Pin<Box<dyn Future<Output = ()>>>)
        });
        let shard_count = shards.len();
        Ok(Swarm { inner: Arc::new(Inner { ctx, spawn, shards: Mutex::new(shards), shard_count }) })
    }
}
