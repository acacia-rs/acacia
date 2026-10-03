//! HTTP downloads of ResourcePacksInfo `cdn_url` packs, as vanilla makes them (2026-10-03 capture
//! with `acacia-mitm --pack-cdn`): a HEAD, then a GET of exactly that URL, each on a new connection
//! with libhttpclient's headers in its order. The body is read and dropped: the pack cache keeps ids
//! only.

use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio_tungstenite::tungstenite::http::Uri;

use crate::signaling::{open_stream, Io};
use crate::socks5::Socks5Proxy;

pub(crate) const FETCH_TIMEOUT: Duration = Duration::from_secs(600);
const USER_AGENT: &str = "libhttpclient/1.0.0.0";
const MAX_REDIRECTS: usize = 5;
const MAX_HEAD: usize = 64 << 10;

/// Downloads the pack at `url`; returns its size.
pub(crate) async fn fetch(url: &str, proxy: Option<&Socks5Proxy>) -> Result<u64, String> {
    request(url, Method::Head, proxy).await?;
    request(url, Method::Get, proxy).await
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Method {
    Head,
    Get,
}

/// Body length after following redirects; HEAD reads none.
async fn request(url: &str, method: Method, proxy: Option<&Socks5Proxy>) -> Result<u64, String> {
    let mut uri: Uri = url.parse().map_err(|e| format!("{url}: {e}"))?;
    for _ in 0..=MAX_REDIRECTS {
        let tls = match uri.scheme_str() {
            Some("https") => true,
            Some("http") => false,
            _ => return Err(format!("{uri}: not http(s)")),
        };
        let host = uri.host().ok_or(format!("{uri}: no host"))?.to_owned();
        let port = uri.port_u16().unwrap_or(if tls { 443 } else { 80 });
        let (stream, _) = open_stream(host, port, tls, proxy).await.map_err(|e| e.to_string())?;
        let mut stream = BufReader::new(stream);
        stream.write_all(request_head(&uri, method).as_bytes()).await.map_err(|e| e.to_string())?;
        let head = read_head(&mut stream).await?;
        match head.status {
            300..=399 => {
                let location = head.header("location").ok_or("redirect without a Location")?;
                uri = redirect(&uri, location)?;
            }
            200..=299 if method == Method::Head => return Ok(0),
            200..=299 => return read_body(&mut stream, &head).await,
            status => return Err(format!("{uri}: HTTP {status}")),
        }
    }
    Err(format!("{url}: too many redirects"))
}

fn request_head(uri: &Uri, method: Method) -> String {
    let path = uri.path_and_query().map_or("/", |p| p.as_str());
    let authority = uri.authority().map_or("", |a| a.as_str());
    match method {
        Method::Head => format!(
            "HEAD {path} HTTP/1.1\r\nConnection: Keep-Alive\r\nAccept-Encoding: identity\r\nUser-Agent: {USER_AGENT}\r\nHost: {authority}\r\n\r\n"
        ),
        Method::Get => format!("GET {path} HTTP/1.1\r\nConnection: Keep-Alive\r\nUser-Agent: {USER_AGENT}\r\nHost: {authority}\r\n\r\n"),
    }
}

fn redirect(from: &Uri, location: &str) -> Result<Uri, String> {
    let target = if location.starts_with('/') {
        format!("{}://{}{location}", from.scheme_str().unwrap_or("https"), from.authority().map_or("", |a| a.as_str()))
    } else {
        location.to_owned()
    };
    target.parse().map_err(|e| format!("redirect to {location}: {e}"))
}

struct Head {
    status: u16,
    /// Names lowercased.
    headers: Vec<(String, String)>,
}

impl Head {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(n, _)| n == name).map(|(_, v)| v.as_str())
    }
}

async fn read_head(stream: &mut BufReader<Box<dyn Io>>) -> Result<Head, String> {
    let mut lines = Vec::new();
    let mut size = 0;
    loop {
        let mut line = String::new();
        let n = stream.read_line(&mut line).await.map_err(|e| e.to_string())?;
        size += n;
        if n == 0 || size > MAX_HEAD {
            return Err("truncated HTTP response head".into());
        }
        let line = line.trim_end();
        if line.is_empty() {
            break;
        }
        lines.push(line.to_owned());
    }
    let status_line = lines.first().ok_or("empty HTTP response")?;
    let status = status_line.split(' ').nth(1).and_then(|s| s.parse().ok()).ok_or(format!("bad status line {status_line:?}"))?;
    let headers = lines[1..]
        .iter()
        .filter_map(|l| l.split_once(':'))
        .map(|(n, v)| (n.trim().to_ascii_lowercase(), v.trim().to_owned()))
        .collect();
    Ok(Head { status, headers })
}

async fn read_body(stream: &mut BufReader<Box<dyn Io>>, head: &Head) -> Result<u64, String> {
    if head.header("transfer-encoding").is_some_and(|t| t.eq_ignore_ascii_case("chunked")) {
        return read_chunked(stream).await;
    }
    match head.header("content-length") {
        Some(len) => {
            let len: u64 = len.parse().map_err(|_| format!("bad Content-Length {len:?}"))?;
            let read = discard(&mut *stream, len).await?;
            if read < len {
                return Err(format!("body ended after {read} of {len} bytes"));
            }
            Ok(read)
        }
        None => discard(stream, u64::MAX).await,
    }
}

async fn read_chunked(stream: &mut BufReader<Box<dyn Io>>) -> Result<u64, String> {
    let mut total = 0;
    loop {
        let mut line = String::new();
        stream.read_line(&mut line).await.map_err(|e| e.to_string())?;
        let size = line.trim_end().split(';').next().unwrap_or_default();
        let size = u64::from_str_radix(size, 16).map_err(|_| format!("bad chunk size {line:?}"))?;
        if size == 0 {
            return Ok(total);
        }
        if discard(&mut *stream, size).await? < size {
            return Err("chunked body ended early".into());
        }
        total += size;
        stream.read_line(&mut String::new()).await.map_err(|e| e.to_string())?;
    }
}

/// Reads and drops up to `len` bytes; fewer only at EOF.
async fn discard<R: AsyncRead + Unpin>(stream: R, len: u64) -> Result<u64, String> {
    match tokio::io::copy(&mut stream.take(len), &mut tokio::io::sink()).await {
        Ok(n) => Ok(n),
        // Many servers close TLS without close_notify once the body is sent.
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => Ok(0),
        Err(e) => Err(e.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use tokio::net::TcpListener;

    use super::*;

    /// Answers each connection with the next canned response; returns the requests it saw.
    async fn serve(responses: Vec<&'static str>) -> (String, tokio::task::JoinHandle<Vec<String>>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let task = tokio::spawn(async move {
            let mut seen = Vec::new();
            for response in responses {
                let (mut conn, _) = listener.accept().await.unwrap();
                let mut buf = vec![0; 4096];
                let n = conn.read(&mut buf).await.unwrap();
                seen.push(String::from_utf8_lossy(&buf[..n]).into_owned());
                conn.write_all(response.as_bytes()).await.unwrap();
            }
            seen
        });
        (url, task)
    }

    #[tokio::test]
    async fn heads_then_gets_like_vanilla() {
        let (url, server) = serve(vec![
            "HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\n",
            "HTTP/1.1 302 Found\r\nLocation: /real.zip\r\nContent-Length: 0\r\n\r\n",
            "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n3\r\nabc\r\n2\r\nde\r\n0\r\n\r\n",
        ])
        .await;
        let size = fetch(&format!("{url}/pack.zip"), None).await.unwrap();
        assert_eq!(size, 5);
        let seen = server.await.unwrap();
        let host = url.trim_start_matches("http://");
        assert_eq!(
            seen[0],
            format!("HEAD /pack.zip HTTP/1.1\r\nConnection: Keep-Alive\r\nAccept-Encoding: identity\r\nUser-Agent: libhttpclient/1.0.0.0\r\nHost: {host}\r\n\r\n")
        );
        assert_eq!(seen[1], format!("GET /pack.zip HTTP/1.1\r\nConnection: Keep-Alive\r\nUser-Agent: libhttpclient/1.0.0.0\r\nHost: {host}\r\n\r\n"));
        assert!(seen[2].starts_with("GET /real.zip "));
    }

    #[tokio::test]
    async fn short_body_fails() {
        let (url, _server) = serve(vec!["HTTP/1.1 200 OK\r\n\r\n", "HTTP/1.1 200 OK\r\nContent-Length: 10\r\n\r\nabc"]).await;
        assert!(fetch(&format!("{url}/p.zip"), None).await.is_err());
    }
}
