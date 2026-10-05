//! The signaling service's WebSocket over our rustls stack, through SOCKS5 CONNECT when proxied.
//! Plain `ws://` URLs (fake hosts in tests) skip TLS.

use std::io;
use std::net::SocketAddr;

use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpStream;
use tokio_rustls::rustls::pki_types::ServerName;
use tokio_rustls::TlsConnector;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::{HeaderName, HeaderValue};
use tokio_tungstenite::WebSocketStream;

use crate::nethernet::tls_config;
use crate::socks5::{self, Socks5Proxy};
use crate::transport::resolve;
use crate::ConnectError;

pub(crate) trait Io: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send> Io for T {}

pub(crate) type Socket = WebSocketStream<Box<dyn Io>>;

pub(crate) fn ws_error(e: impl std::fmt::Display) -> ConnectError {
    ConnectError::Signaling(format!("WebSocket: {e}"))
}

/// Opens the socket; also returns the server's address. `headers` names must be lowercase (they go
/// out as given).
pub(crate) async fn connect(
    url: &str,
    headers: &[(&'static str, String)],
    proxy: Option<&Socks5Proxy>,
) -> Result<(Socket, SocketAddr), ConnectError> {
    let mut request = url.into_client_request().map_err(ws_error)?;
    let uri = request.uri();
    let tls = uri.scheme_str() != Some("ws");
    let host = uri.host().ok_or_else(|| ws_error(format!("no host in {url}")))?.to_owned();
    let port = uri.port_u16().unwrap_or(if tls { 443 } else { 80 });
    for (name, value) in headers {
        let value = HeaderValue::from_str(value).map_err(ws_error)?;
        request.headers_mut().insert(HeaderName::from_static(name), value);
    }
    let (stream, addr) = open(host, port, tls, proxy).await?;
    let (socket, _) = tokio_tungstenite::client_async(request, stream).await.map_err(ws_error)?;
    Ok((socket, addr))
}

/// A TCP stream to `host:port`, through the proxy if any, wrapped in TLS (SNI `host`) if `tls`.
pub(crate) async fn open(host: String, port: u16, tls: bool, proxy: Option<&Socks5Proxy>) -> Result<(Box<dyn Io>, SocketAddr), ConnectError> {
    let addr = resolve(&format!("{host}:{port}")).await?;
    let tcp = match proxy {
        Some(proxy) => socks5::connect_tcp(proxy, addr).await?,
        None => TcpStream::connect(addr).await?,
    };
    let stream: Box<dyn Io> = if tls {
        let name = ServerName::try_from(host).map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
        Box::new(TlsConnector::from(tls_config()).connect(name, tcp).await?)
    } else {
        Box::new(tcp)
    };
    Ok((stream, addr))
}
