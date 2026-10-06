//! [`Proxy`]: what to listen on, where to dial, how to log in, and what to do with the packets.

use std::io;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use acacia_auth::{Account, AuthClient, LoginCredentials};
use p384::ecdsa::SigningKey;
use tokio::net::{TcpListener, UdpSocket};

use crate::intercept::{Chain, Interceptor, Session};
use crate::record::{self, DatagramLog, Recorder, SessionLog, SharedRecorder};
use crate::relay::{Relay, Wire};
use crate::{nethernet, raknet};

type Factory = Box<dyn Fn(&Session) -> Box<dyn Interceptor> + Send + Sync>;

/// Where a game that joins the proxy is taken.
#[derive(Clone)]
pub(crate) enum Target {
    Address(SocketAddr),
    /// Asked of Realms for each join: it answers with an address or a signaling target.
    Realm { auth: Arc<AuthClient>, id: i64 },
}

/// What every player's relay is built from.
pub(crate) struct Setup {
    pub target: Target,
    pub rec: Option<SharedRecorder>,
    pub follow_transfers: bool,
    sessions: AtomicU32,
    factories: Vec<Factory>,
}

impl Setup {
    /// The relay for a new player, with fresh interceptors and the capture's next session number.
    /// `wires` are the game's and the server's.
    pub fn relay(&self, wires: (Wire, Wire), key: SigningKey, credentials: Option<LoginCredentials>, session: &Session) -> Relay {
        let chain = Chain::new(self.factories.iter().map(|f| f(session)).collect());
        let log = self.rec.clone().map(|rec| SessionLog::new(rec, self.sessions.fetch_add(1, Ordering::Relaxed)));
        Relay::new(wires, key, credentials, log, chain)
    }

    /// The server's address, when the target is one.
    pub fn address(&self) -> Option<SocketAddr> {
        match self.target {
            Target::Address(server) => Some(server),
            Target::Realm { .. } => None,
        }
    }
}

enum Transport {
    RakNet,
    /// The key signs our signaling answers; the game pins it per server address.
    NetherNet(SigningKey),
}

/// A man-in-the-middle proxy for one server. The game joins the proxy, which joins the server as
/// the same player (re-signing Login: offline, or as the [`online`](Self::online) account).
///
/// ```no_run
/// # async fn t() -> std::io::Result<()> {
/// use acacia_mitm::{Interceptor, Proxy, Verdict};
/// use acacia_mitm::proto::{packets::SetTime, RawPacket};
///
/// struct NoTime;
/// impl Interceptor for NoTime {
///     fn on_server_packet(&mut self, packet: &RawPacket) -> Verdict {
///         if packet.is::<SetTime>() { Verdict::Drop } else { Verdict::Forward }
///     }
/// }
/// Proxy::new("127.0.0.1:19140".parse().unwrap()).intercept(|_| NoTime).bind().await?.run().await
/// # }
/// ```
pub struct Proxy {
    listen: SocketAddr,
    transport: Transport,
    account: Option<Account>,
    trace_datagrams: bool,
    setup: Setup,
}

impl Proxy {
    /// Listens on 0.0.0.0:19180 over RakNet, offline, following transfers, without recording or
    /// interceptors.
    pub fn new(server: SocketAddr) -> Self {
        Self {
            listen: SocketAddr::from(([0, 0, 0, 0], 19180)),
            transport: Transport::RakNet,
            account: None,
            trace_datagrams: false,
            setup: Setup { target: Target::Address(server), rec: None, follow_transfers: true, sessions: AtomicU32::new(0), factories: Vec::new() },
        }
    }

    /// Like [`new`](Self::new), for a realm `account` is a member of: every game that joins is
    /// taken to it, over RakNet or through the signaling service as Realms says. The game still
    /// joins the proxy by address over RakNet. `auth` is the client `account` was made with.
    pub fn realm(auth: Arc<AuthClient>, account: Account, realm_id: i64) -> Self {
        let mut proxy = Self::new(SocketAddr::from(([0, 0, 0, 0], 0))).online(account);
        proxy.setup.target = Target::Realm { auth, id: realm_id };
        proxy
    }

    /// Over RakNet: whether a Transfer the interceptors let through is pointed back at the proxy,
    /// which then dials its target for that game (on by default). Off, the game leaves the proxy.
    pub fn follow_transfers(mut self, on: bool) -> Self {
        self.setup.follow_transfers = on;
        self
    }

    pub fn listen(mut self, addr: SocketAddr) -> Self {
        self.listen = addr;
        self
    }

    /// Joins the server as this signed-in account. Sign the game into the same one: its client
    /// data passes through.
    pub fn online(mut self, account: Account) -> Self {
        self.account = Some(account);
        self
    }

    /// NetherNet direct connect instead of RakNet: serves BDS-style signaling on `listen` (TCP) and
    /// dials the server the same way. Needs [`online`](Self::online).
    pub fn nethernet(mut self, host_key: SigningKey) -> Self {
        self.transport = Transport::NetherNet(host_key);
        self
    }

    /// Logs every player's packets, as they arrived and before interceptors, to `rec`.
    pub fn record(mut self, rec: Recorder) -> Self {
        self.setup.rec = Some(Arc::new(Mutex::new(rec)));
        self
    }

    /// Over RakNet with [`record`](Self::record): also logs every game-side datagram's first byte
    /// and size next to the capture.
    pub fn trace_datagrams(mut self, on: bool) -> Self {
        self.trace_datagrams = on;
        self
    }

    /// Adds an interceptor, made fresh for each player, after those added before it.
    pub fn intercept<I: Interceptor>(mut self, factory: impl Fn(&Session) -> I + Send + Sync + 'static) -> Self {
        self.setup.factories.push(Box::new(move |s| Box::new(factory(s))));
        self
    }

    pub async fn bind(self) -> io::Result<BoundProxy> {
        let listener = match (self.transport, self.account) {
            (Transport::RakNet, account) => {
                let trace = match (&self.setup.rec, self.trace_datagrams) {
                    (Some(rec), true) => Some(record::lock(rec).datagram_log()?),
                    _ => None,
                };
                Listener::RakNet { socket: UdpSocket::bind(self.listen).await?, account, trace }
            }
            (Transport::NetherNet(_), _) if self.setup.address().is_none() => {
                let why = "a realm is reached with the game joining the proxy over RakNet, not NetherNet";
                return Err(io::Error::new(io::ErrorKind::InvalidInput, why));
            }
            (Transport::NetherNet(key), Some(account)) => Listener::NetherNet { tcp: TcpListener::bind(self.listen).await?, account, key },
            (Transport::NetherNet(_), None) => {
                let why = "NetherNet needs an online account: BDS refuses NetherNet offers without a MultiplayerToken";
                return Err(io::Error::new(io::ErrorKind::InvalidInput, why));
            }
        };
        Ok(BoundProxy { listener, setup: self.setup })
    }
}

enum Listener {
    RakNet { socket: UdpSocket, account: Option<Account>, trace: Option<DatagramLog> },
    NetherNet { tcp: TcpListener, account: Account, key: SigningKey },
}

/// A [`Proxy`] holding its socket, so the address is known before players join.
pub struct BoundProxy {
    listener: Listener,
    setup: Setup,
}

impl BoundProxy {
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        match &self.listener {
            Listener::RakNet { socket, .. } => socket.local_addr(),
            Listener::NetherNet { tcp, .. } => tcp.local_addr(),
        }
    }

    /// Serves players until an I/O error on the listening socket.
    pub async fn run(self) -> io::Result<()> {
        match self.listener {
            Listener::RakNet { socket, account, trace } => raknet::run(socket, self.setup, account, trace).await,
            Listener::NetherNet { tcp, account, key } => nethernet::run(tcp, nethernet::Host { setup: self.setup, account, key }).await,
        }
    }
}
