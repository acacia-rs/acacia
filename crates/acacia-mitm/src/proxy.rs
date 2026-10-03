//! [`Proxy`]: what to listen on, where to dial, how to log in, and what to do with the packets.

use std::io;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use acacia_auth::{Account, LoginCredentials};
use p384::ecdsa::SigningKey;
use tokio::net::{TcpListener, UdpSocket};

use crate::intercept::{Chain, Interceptor, Session};
use crate::record::{self, DatagramLog, Recorder, SharedRecorder};
use crate::relay::{Relay, Wire};
use crate::{nethernet, raknet};

type Factory = Box<dyn Fn(&Session) -> Box<dyn Interceptor> + Send + Sync>;

/// What every player's relay is built from.
pub(crate) struct Setup {
    pub server: SocketAddr,
    pub rec: Option<SharedRecorder>,
    factories: Vec<Factory>,
}

impl Setup {
    /// The relay for a new player, with fresh interceptors.
    pub fn relay(&self, wire: Wire, key: SigningKey, credentials: Option<LoginCredentials>, session: &Session) -> Relay {
        let chain = Chain::new(self.factories.iter().map(|f| f(session)).collect());
        Relay::new(wire, key, credentials, self.rec.clone(), chain)
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
    /// Listens on 0.0.0.0:19180 over RakNet, offline, without recording or interceptors.
    pub fn new(server: SocketAddr) -> Self {
        Self {
            listen: SocketAddr::from(([0, 0, 0, 0], 19180)),
            transport: Transport::RakNet,
            account: None,
            trace_datagrams: false,
            setup: Setup { server, rec: None, factories: Vec::new() },
        }
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
