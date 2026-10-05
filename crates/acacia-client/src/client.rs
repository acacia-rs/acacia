use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use acacia_auth::LoginCredentials;
use acacia_session::blob_store::BlobStore;
use acacia_session::{DisconnectReason, Session, SessionConfig};
use acacia_nethernet::Identity as NetIdentity;
use p384::ecdsa::SigningKey;
use tokio::sync::{mpsc, oneshot};

use crate::blob_cache::BlobCache;
use crate::driver::{Command, Driver};
use crate::friend::FriendJoin;
use crate::pack_cache;
use crate::filter::PacketFilter;
use crate::login::build_login;
use crate::route;
use crate::lan::LanServer;
use crate::signaling::SignalingTarget;
use crate::socks5::Socks5Proxy;
use crate::transport::resolve;
use crate::{Client, ConnectError};

/// Which transport to join over.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TransportKind {
    /// RakNet if the server answers a RakNet ping, else NetherNet if it answers `GET /v1/join`.
    /// Offline logins always use RakNet.
    #[default]
    Auto,
    RakNet,
    /// WebRTC direct connect. An offline login offers a self-signed identity, which BDS refuses
    /// (error 37) and a host without online auth may accept.
    NetherNet,
}

pub enum Login {
    /// For `online-mode=false` servers.
    Offline { name: String },
    /// `credentials` must have been obtained for `key` (acacia-auth binds tokens to the client key).
    Online { credentials: Box<LoginCredentials>, key: SigningKey },
}

/// How a join reaches the world.
enum Via {
    Address,
    Signaling(SignalingTarget),
    Lan(LanServer),
}

pub struct ClientBuilder {
    server: String,
    via: Via,
    /// The Login `Nonce` a friend's world handed out.
    nonce: Option<String>,
    /// Held by the connection; dropping it leaves the friend's Xbox session.
    friend_session: Option<oneshot::Sender<()>>,
    blob_cache: BlobCache,
    blob_payloads: bool,
    pack_cache_dir: Option<PathBuf>,
    login: Login,
    proxy: Option<Socks5Proxy>,
    transport: TransportKind,
    chunk_radius: i32,
    auto_respawn: bool,
    initialize_on_spawn: bool,
    strict: bool,
    filter: PacketFilter,
    event_capacity: usize,
    login_timeout: Duration,
}

impl ClientBuilder {
    pub fn new(server: impl Into<String>) -> Self {
        Self {
            server: server.into(),
            via: Via::Address,
            nonce: None,
            friend_session: None,
            blob_cache: BlobCache::Memory,
            blob_payloads: false,
            pack_cache_dir: None,
            login: Login::Offline { name: "Player".into() },
            proxy: None,
            transport: TransportKind::Auto,
            chunk_radius: 2,
            auto_respawn: false,
            initialize_on_spawn: true,
            strict: false,
            filter: PacketFilter::all(),
            event_capacity: 256,
            login_timeout: Duration::from_secs(30),
        }
    }

    /// The server as given to [`ClientBuilder::new`].
    pub fn server(&self) -> &str {
        &self.server
    }

    pub fn offline(mut self, name: impl Into<String>) -> Self {
        self.login = Login::Offline { name: name.into() };
        self
    }

    pub fn online(mut self, credentials: LoginCredentials, key: SigningKey) -> Self {
        self.login = Login::Online { credentials: Box::new(credentials), key };
        self
    }

    /// Joins through the signaling service (e.g. a NetherNet realm) instead of dialing `server`.
    pub fn signaling(mut self, target: SignalingTarget) -> Self {
        self.via = Via::Signaling(target);
        self
    }

    /// Joins a world found by [`crate::discover_lan`] instead of dialing `server`.
    pub fn lan(mut self, server: LanServer) -> Self {
        self.via = Via::Lan(server);
        self
    }

    /// Joins a friend's world (see `join_friend_world`) through the signaling service, with its nonce.
    /// The connection keeps the Xbox session membership until it ends.
    pub fn friend(mut self, join: FriendJoin) -> Self {
        self.via = Via::Signaling(join.target);
        self.nonce = join.nonce;
        self.friend_session = Some(join.session);
        self
    }

    pub fn proxy(mut self, proxy: Socks5Proxy) -> Self {
        self.proxy = Some(proxy);
        self
    }

    pub fn transport(mut self, kind: TransportKind) -> Self {
        self.transport = kind;
        self
    }

    /// Respawn automatically whenever the player dies after spawning.
    pub fn auto_respawn(mut self, enabled: bool) -> Self {
        self.auto_respawn = enabled;
        self
    }

    /// Send SetLocalPlayerAsInitialized on spawn (default). Off when the caller sends it at the end
    /// of its loading-screen sequence, as vanilla does.
    pub fn initialize_on_spawn(mut self, enabled: bool) -> Self {
        self.initialize_on_spawn = enabled;
        self
    }

    /// Strict mode: every server packet is fully decoded, and what a strict peer would reject
    /// arrives as [`crate::Event::Violation`] whatever the packet filter (docs/testing.md).
    pub fn strict(mut self, enabled: bool) -> Self {
        self.strict = enabled;
        self
    }

    /// Requested view distance in chunks; small values cut most of an idle bot's inbound traffic.
    pub fn chunk_radius(mut self, radius: i32) -> Self {
        self.chunk_radius = radius;
        self
    }

    /// Deliver only these packet IDs (default: all).
    pub fn subscribe(mut self, ids: impl IntoIterator<Item = u32>) -> Self {
        self.filter = ids.into_iter().collect();
        self
    }

    pub fn filter(mut self, filter: PacketFilter) -> Self {
        self.filter = filter;
        self
    }

    /// Max undelivered events before the connection stops reading (backpressure).
    pub fn event_capacity(mut self, capacity: usize) -> Self {
        self.event_capacity = capacity.max(1);
        self
    }

    pub fn login_timeout(mut self, timeout: Duration) -> Self {
        self.login_timeout = timeout;
        self
    }

    /// Keep the blob cache in `dir`, one file per account, across joins (default: per connection).
    pub fn blob_cache_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.blob_cache = BlobCache::Dir(dir.into());
        self
    }

    pub fn blob_store(mut self, store: Arc<dyn BlobStore>) -> Self {
        self.blob_cache = BlobCache::Store(store);
        self
    }

    /// Report the blob cache as disabled, as before vanilla's cache was copied (not vanilla).
    pub fn without_blob_cache(mut self) -> Self {
        self.blob_cache = BlobCache::Off;
        self
    }

    /// Remember downloaded resource packs in `dir`, one file per account, so later joins skip the
    /// download like a vanilla player's cache (default: every join downloads).
    pub fn pack_cache_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.pack_cache_dir = Some(dir.into());
        self
    }

    /// Keep blob bytes, not just hashes: needed to read terrain ([`Client::blob_store`]).
    pub fn keep_blob_payloads(mut self, keep: bool) -> Self {
        self.blob_payloads = keep;
        self
    }

    /// Connects, logs in and resolves once the player has spawned.
    pub async fn connect(self) -> Result<Client, ConnectError> {
        // Made here, not with the Login: a NetherNet offer is signed by the same key.
        let offline_key = SigningKey::random(&mut rand_core::OsRng);
        let net_identity = match &self.login {
            Login::Online { credentials, key } => {
                credentials.multiplayer_token.clone().map(|token| NetIdentity::multiplayer(key.clone(), token))
            }
            Login::Offline { .. } => Some(NetIdentity::self_signed(offline_key.clone())),
        };
        let transport = match (&self.login, self.transport) {
            (Login::Offline { .. }, TransportKind::Auto) => TransportKind::RakNet,
            (_, kind) => kind,
        };
        let route = match &self.via {
            Via::Signaling(target) => route::open_signaling(target, self.proxy.as_ref(), net_identity.as_ref()).await?,
            Via::Lan(server) => route::open_lan(server, net_identity.as_ref()).await?,
            Via::Address => {
                let addr = resolve(&self.server).await?;
                route::open(&self.server, addr, self.proxy.as_ref(), transport, net_identity.as_ref()).await?
            }
        };
        let addr = route.remote;
        let account = match &self.login {
            Login::Offline { name } => format!("offline:{name}"),
            Login::Online { credentials, .. } => format!("xbox:{}", credentials.xuid),
        };
        let blob_store = self.blob_cache.open(&account, self.blob_payloads);
        let (key, login_request, identity) = build_login(self.login, offline_key, &route.server_address, self.nonce);
        let cfg = SessionConfig {
            link: route.link,
            key,
            login_request,
            chunk_radius: self.chunk_radius,
            auto_respawn: self.auto_respawn,
            initialize_on_spawn: self.initialize_on_spawn,
            blob_store: blob_store.clone(),
            pack_store: pack_cache::open(self.pack_cache_dir.as_deref(), &account),
            strict: self.strict,
        };
        let session = Session::new(cfg, addr, Instant::now());

        let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
        let (ev_tx, ev_rx) = mpsc::channel(self.event_capacity);
        let (spawn_tx, spawn_rx) = oneshot::channel();
        let driver = Driver {
            session,
            wire: route.wire,
            transport: route.transport,
            commands: cmd_rx,
            events: ev_tx,
            spawned: Some(spawn_tx),
            filter: self.filter,
            capacity: self.event_capacity,
            proxy: self.proxy.clone(),
            _signaling: route.keepalive,
            _friend_session: self.friend_session,
        };
        tokio::spawn(driver.run());

        let result = tokio::time::timeout(self.login_timeout, spawn_rx).await;
        match result {
            Ok(Ok(Ok(runtime_entity_id))) => Ok(Client { commands: cmd_tx, events: ev_rx, runtime_entity_id, identity, blob_store }),
            Ok(Ok(Err(reason))) => Err(ConnectError::Disconnected(reason)),
            Ok(Err(_)) => Err(ConnectError::Disconnected(DisconnectReason::LocalClose)),
            Err(_) => {
                let _ = cmd_tx.send(Command::Close);
                Err(ConnectError::Timeout)
            }
        }
    }
}
