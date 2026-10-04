use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use acacia_auth::LoginCredentials;
use acacia_session::blob_store::BlobStore;
use acacia_session::proto::{encode_packet, Packet};
use acacia_session::{DisconnectReason, Session, SessionConfig};
use acacia_nethernet::Identity as NetIdentity;
use bytes::{Bytes, BytesMut};
use p384::ecdsa::SigningKey;
use tokio::sync::{mpsc, oneshot};

use crate::blob_cache::BlobCache;
use crate::driver::{Command, Driver};
use crate::pack_cache;
use crate::filter::PacketFilter;
use crate::login::{build_login, Identity};
use crate::route;
use crate::lan::LanServer;
use crate::signaling::SignalingTarget;
use crate::socks5::Socks5Proxy;
use crate::transport::resolve;
use crate::{ConnectError, Event};

/// Which transport to join over.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TransportKind {
    /// RakNet if the server answers a RakNet ping, else NetherNet if it answers `GET /v1/join`.
    /// Offline logins always use RakNet.
    #[default]
    Auto,
    RakNet,
    /// WebRTC direct connect; needs an online login.
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
    blob_cache: BlobCache,
    blob_payloads: bool,
    pack_cache_dir: Option<PathBuf>,
    login: Login,
    proxy: Option<Socks5Proxy>,
    transport: TransportKind,
    chunk_radius: i32,
    auto_respawn: bool,
    initialize_on_spawn: bool,
    filter: PacketFilter,
    event_capacity: usize,
    login_timeout: Duration,
}

impl ClientBuilder {
    pub fn new(server: impl Into<String>) -> Self {
        Self {
            server: server.into(),
            via: Via::Address,
            blob_cache: BlobCache::Memory,
            blob_payloads: false,
            pack_cache_dir: None,
            login: Login::Offline { name: "Player".into() },
            proxy: None,
            transport: TransportKind::Auto,
            chunk_radius: 2,
            auto_respawn: false,
            initialize_on_spawn: true,
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
        let net_identity = match &self.login {
            Login::Online { credentials, key } => {
                credentials.multiplayer_token.clone().map(|token| NetIdentity::multiplayer(key.clone(), token))
            }
            Login::Offline { .. } => None,
        };
        let route = match &self.via {
            Via::Signaling(target) => route::open_signaling(target, self.proxy.as_ref(), net_identity.as_ref()).await?,
            Via::Lan(server) => route::open_lan(server, net_identity.as_ref()).await?,
            Via::Address => {
                let addr = resolve(&self.server).await?;
                route::open(&self.server, addr, self.proxy.as_ref(), self.transport, net_identity.as_ref()).await?
            }
        };
        let addr = route.remote;
        let account = match &self.login {
            Login::Offline { name } => format!("offline:{name}"),
            Login::Online { credentials, .. } => format!("xbox:{}", credentials.xuid),
        };
        let blob_store = self.blob_cache.open(&account, self.blob_payloads);
        let (key, login_request, identity) = build_login(self.login, &route.server_address);
        let cfg = SessionConfig {
            link: route.link,
            key,
            login_request,
            chunk_radius: self.chunk_radius,
            auto_respawn: self.auto_respawn,
            initialize_on_spawn: self.initialize_on_spawn,
            blob_store: blob_store.clone(),
            pack_store: pack_cache::open(self.pack_cache_dir.as_deref(), &account),
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

/// A spawned connection. Dropping it disconnects.
pub struct Client {
    commands: mpsc::UnboundedSender<Command>,
    events: mpsc::Receiver<Event>,
    runtime_entity_id: u64,
    identity: Identity,
    blob_store: Option<Arc<dyn BlobStore>>,
}

impl Client {
    /// The blob cache this connection reports from; terrain readers fetch blob payloads here.
    pub fn blob_store(&self) -> Option<&Arc<dyn BlobStore>> {
        self.blob_store.as_ref()
    }

    pub fn builder(server: impl Into<String>) -> ClientBuilder {
        ClientBuilder::new(server)
    }

    pub fn runtime_entity_id(&self) -> u64 {
        self.runtime_entity_id
    }

    pub fn display_name(&self) -> &str {
        &self.identity.display_name
    }

    /// Empty for offline logins.
    pub fn xuid(&self) -> &str {
        &self.identity.xuid
    }

    /// Next packet or the final disconnect; `None` once the connection is gone.
    pub async fn recv(&mut self) -> Option<Event> {
        self.events.recv().await
    }

    /// Queues a packet; returns false if the connection has closed.
    pub fn send<T: Packet>(&self, packet: &T) -> bool {
        let mut buf = BytesMut::new();
        encode_packet(packet, &mut buf);
        self.send_raw(buf.freeze())
    }

    /// Queues an already-encoded packet (header + body).
    pub fn send_raw(&self, packet: Bytes) -> bool {
        self.command_raw(Command::Send(packet))
    }

    pub fn close(&self) {
        self.command_raw(Command::Close);
    }

    pub(crate) fn command_raw(&self, command: Command) -> bool {
        self.commands.send(command).is_ok()
    }
}
