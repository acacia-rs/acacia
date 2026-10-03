//! NetherNet dialing: HTTP(S) signaling on the server's TCP game port, then a WebRTC connection
//! that the driver runs. Like the vanilla client, HTTPS is tried first, then plain HTTP (what BDS
//! serves). Wire spec: docs/research/nethernet-wire.md.

use std::io;
use std::net::SocketAddr;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use acacia_nethernet::{http, Connection, Identity};
use rand_core::{OsRng, RngCore};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::timeout;
use tokio_rustls::rustls::pki_types::ServerName;
use tokio_rustls::rustls::{self, ClientConfig, RootCertStore};
use tokio_rustls::TlsConnector;

use crate::socks5::{self, Socks5Proxy};
use crate::transport::Transport;
use crate::ConnectError;

const SIGNALING_TIMEOUT: Duration = Duration::from_secs(15);
const PROBE_TIMEOUT: Duration = Duration::from_secs(3);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Scheme {
    Https,
    Http,
}

impl Scheme {
    const PREFERENCE: [Scheme; 2] = [Scheme::Https, Scheme::Http];

    fn name(self) -> &'static str {
        match self {
            Self::Https => "https",
            Self::Http => "http",
        }
    }
}

/// Where to signal: the resolved address plus the name the user gave (for SNI and `Host`).
pub(crate) struct Endpoint<'a> {
    pub addr: SocketAddr,
    pub host: &'a str,
    pub proxy: Option<&'a Socks5Proxy>,
}

impl Endpoint<'_> {
    fn authority(&self) -> String {
        format!("{}:{}", self.host, self.addr.port())
    }

    /// The Login `ServerAddress` for a URL-addressed join: `<scheme>://host:port:port`.
    pub fn server_address(&self, scheme: Scheme) -> String {
        format!("{}://{}:{}", scheme.name(), self.authority(), self.addr.port())
    }

    /// Sends one request and reads the response until the server closes the connection.
    async fn exchange(&self, scheme: Scheme, request: &[u8]) -> io::Result<Vec<u8>> {
        let tcp = match self.proxy {
            Some(proxy) => socks5::connect_tcp(proxy, self.addr).await?,
            None => TcpStream::connect(self.addr).await?,
        };
        match scheme {
            Scheme::Http => roundtrip(tcp, request).await,
            Scheme::Https => {
                let name = ServerName::try_from(self.host.to_owned()).map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
                roundtrip(TlsConnector::from(tls_config()).connect(name, tcp).await?, request).await
            }
        }
    }
}

/// Sends `request` and reads one response: up to its Content-Length, or EOF if it has none.
async fn roundtrip<S: AsyncRead + AsyncWrite + Unpin>(mut stream: S, request: &[u8]) -> io::Result<Vec<u8>> {
    stream.write_all(request).await?;
    let mut response = Vec::new();
    let mut chunk = [0u8; 8192];
    while !http::response_complete(&response) && response.len() < http::MAX_MESSAGE {
        match stream.read(&mut chunk).await {
            Ok(0) => break,
            Ok(n) => response.extend_from_slice(&chunk[..n]),
            // Many servers close TLS without close_notify once the body is sent.
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof && !response.is_empty() => break,
            Err(e) => return Err(e),
        }
    }
    Ok(response)
}

pub(crate) fn tls_config() -> Arc<ClientConfig> {
    static CONFIG: OnceLock<Arc<ClientConfig>> = OnceLock::new();
    CONFIG
        .get_or_init(|| {
            let roots = RootCertStore { roots: webpki_roots::TLS_SERVER_ROOTS.to_vec() };
            let config = ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
                .with_safe_default_protocol_versions()
                .expect("ring supports the default TLS versions")
                .with_root_certificates(roots)
                .with_no_client_auth();
            Arc::new(config)
        })
        .clone()
}

/// The scheme on which the server answers `GET /v1/join` with a 2xx, i.e. accepts NetherNet.
pub(crate) async fn probe(endpoint: &Endpoint<'_>) -> Option<Scheme> {
    let request = http::probe_request(&endpoint.authority());
    for scheme in Scheme::PREFERENCE {
        if let Ok(Ok(raw)) = timeout(PROBE_TIMEOUT, endpoint.exchange(scheme, &request)).await
            && http::parse_response(&raw).is_ok_and(|(status, _)| (200..300).contains(&status))
        {
            return Some(scheme);
        }
    }
    None
}

/// Like the vanilla client: probes (unless `scheme` is already known), posts the offer on the scheme
/// that answered and accepts the answer; ICE, DTLS and the data channels then come up as the driver
/// feeds the connection.
pub(crate) async fn dial(
    endpoint: &Endpoint<'_>,
    scheme: Option<Scheme>,
    identity: &Identity,
) -> Result<(Transport, Connection, Scheme), ConnectError> {
    let scheme = match scheme {
        Some(scheme) => scheme,
        None => probe(endpoint).await.ok_or(ConnectError::NetherNetUnsupported)?,
    };
    let transport = Transport::bind(endpoint.addr, endpoint.proxy).await?;
    let (mut conn, offer) = Connection::offer(transport.local_candidate()?, identity, Instant::now())?;
    let request = http::offer_request(&endpoint.authority(), OsRng.next_u64(), &offer);
    tracing::debug!(server = %endpoint.addr, local = ?conn.host_candidate(), ?scheme, "posting NetherNet offer");
    let raw = timeout(SIGNALING_TIMEOUT, endpoint.exchange(scheme, &request)).await.map_err(|_| ConnectError::Timeout)??;
    let answer = http::answer_sdp(&raw)?;
    tracing::debug!(candidates = answer.matches("a=candidate").count(), "got NetherNet answer");
    conn.accept_answer(&answer, Instant::now())?;
    Ok((transport, conn, scheme))
}
