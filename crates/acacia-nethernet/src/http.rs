//! The two HTTP/1.1 exchanges of direct-connect signaling, as bytes: the vanilla client's requests
//! with its headers in its order, and BDS's responses. The client asks for keep-alive, so a
//! response ends at Content-Length, not at EOF.

use crate::Error;

/// Signaling requests and responses are capped at 1 MiB (go-nethernet's server limit).
pub const MAX_MESSAGE: usize = 1 << 20;

pub fn probe_request(host: &str) -> Vec<u8> {
    request("GET", host, crate::PROBE_PATH, None)
}

pub fn offer_request(host: &str, network_id: u64, sdp: &str) -> Vec<u8> {
    request("POST", host, &crate::join_path(network_id), Some(("application/sdp", sdp)))
}

fn request(method: &str, host: &str, path: &str, body: Option<(&str, &str)>) -> Vec<u8> {
    let mut out = format!("{method} {path} HTTP/1.1\r\nConnection: Keep-Alive\r\n");
    match body {
        Some((content_type, body)) => out.push_str(&format!(
            "Content-Type: {content_type}\r\nUser-Agent: libhttpclient/1.0.0.0\r\nContent-Length: {}\r\nHost: {host}\r\n\r\n{body}",
            body.len()
        )),
        None => out.push_str(&format!("User-Agent: libhttpclient/1.0.0.0\r\nHost: {host}\r\n\r\n")),
    }
    out.into_bytes()
}

/// Where the body starts, and the Content-Length if there is one, once the headers are in.
fn head(raw: &[u8]) -> Option<(usize, Option<usize>)> {
    let split = raw.windows(4).position(|w| w == b"\r\n\r\n")?;
    let head = String::from_utf8_lossy(&raw[..split]).to_ascii_lowercase();
    let len = head.lines().find_map(|l| l.strip_prefix("content-length:")).and_then(|v| v.trim().parse().ok());
    Some((split + 4, len))
}

/// True once `raw` holds the headers and the whole Content-Length body (no length: wait for EOF).
pub fn response_complete(raw: &[u8]) -> bool {
    head(raw).is_some_and(|(body, len)| len.is_some_and(|len| raw.len() >= body + len))
}

/// True once `raw` holds a request's headers and its Content-Length body (none without a length).
pub fn request_complete(raw: &[u8]) -> bool {
    head(raw).is_some_and(|(body, len)| raw.len() >= body + len.unwrap_or(0))
}

/// The vanilla client tries HTTPS first; BDS's plain-HTTP port answers its ClientHello with this
/// fatal `handshake_failure` alert, and the client retries over HTTP.
pub const TLS_REFUSAL: [u8; 7] = [0x15, 0x03, 0x01, 0x00, 0x02, 0x02, 0x28];

/// True if `raw` starts with a TLS handshake record (a ClientHello) rather than an HTTP request.
pub fn is_tls(raw: &[u8]) -> bool {
    raw.first() == Some(&0x16)
}

pub struct Request<'a> {
    pub method: &'a str,
    pub path: &'a str,
    pub body: &'a [u8],
}

/// Method, path and body of a complete request.
pub fn parse_request(raw: &[u8]) -> Result<Request<'_>, Error> {
    let (body, _) = head(raw).ok_or(Error::MalformedHttp)?;
    let line = std::str::from_utf8(&raw[..body]).map_err(|_| Error::MalformedHttp)?.lines().next().unwrap_or_default();
    let mut parts = line.split(' ');
    let (Some(method), Some(path)) = (parts.next(), parts.next()) else { return Err(Error::MalformedHttp) };
    Ok(Request { method, path, body: &raw[body..] })
}

/// A response the way BDS writes one: Content-Type, Content-Length, `Connection: close`.
pub fn response(status: u16, content_type: &str, body: &str) -> Vec<u8> {
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        503 => "Service Unavailable",
        _ => "Error",
    };
    format!("HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).into_bytes()
}

/// Status code and body of a complete response.
pub fn parse_response(raw: &[u8]) -> Result<(u16, &[u8]), Error> {
    let split = raw.windows(4).position(|w| w == b"\r\n\r\n").ok_or(Error::MalformedHttp)?;
    let head = std::str::from_utf8(&raw[..split]).map_err(|_| Error::MalformedHttp)?;
    let status = head.split(' ').nth(1).and_then(|s| s.parse().ok()).ok_or(Error::MalformedHttp)?;
    Ok((status, &raw[split + 4..]))
}

/// The SDP answer from a `POST /v1/join` response. Servers report refusals as a bare decimal
/// error code, sometimes inside a 2xx.
pub fn answer_sdp(raw: &[u8]) -> Result<String, Error> {
    let (status, body) = parse_response(raw)?;
    let body = String::from_utf8_lossy(body).trim().to_owned();
    if let Ok(code) = body.parse::<u32>() {
        return Err(Error::Signaling { code });
    }
    if !(200..300).contains(&status) {
        return Err(Error::Http { status, body });
    }
    Ok(body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn answer_variants() {
        let ok = b"HTTP/1.1 200 OK\r\nContent-Type: application/sdp\r\n\r\nv=0\r\n";
        assert_eq!(answer_sdp(ok).unwrap(), "v=0");
        let refused = b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n37";
        assert!(matches!(answer_sdp(refused), Err(Error::Signaling { code: 37 })));
        let missing = b"HTTP/1.1 404 Not Found\r\n\r\nnope";
        assert!(matches!(answer_sdp(missing), Err(Error::Http { status: 404, .. })));
        assert!(parse_response(b"garbage").is_err());
    }

    #[test]
    fn requests_match_vanilla_header_order() {
        let get = String::from_utf8(probe_request("1.2.3.4:19132")).unwrap();
        assert_eq!(get, "GET /v1/join HTTP/1.1\r\nConnection: Keep-Alive\r\nUser-Agent: libhttpclient/1.0.0.0\r\nHost: 1.2.3.4:19132\r\n\r\n");
        let post = String::from_utf8(offer_request("1.2.3.4:19132", 7, "v=0\r\n")).unwrap();
        assert_eq!(
            post,
            "POST /v1/join/7 HTTP/1.1\r\nConnection: Keep-Alive\r\nContent-Type: application/sdp\r\nUser-Agent: libhttpclient/1.0.0.0\r\nContent-Length: 5\r\nHost: 1.2.3.4:19132\r\n\r\nv=0\r\n"
        );
    }

    #[test]
    fn serves_the_vanilla_requests() {
        let get = probe_request("192.168.1.236:19170");
        assert!(request_complete(&get) && !is_tls(&get));
        let req = parse_request(&get).unwrap();
        assert_eq!((req.method, req.path, req.body), ("GET", "/v1/join", &b""[..]));
        let post = offer_request("192.168.1.236:19170", 7, "v=0\r\n");
        assert!(!request_complete(&post[..post.len() - 1]) && request_complete(&post));
        let req = parse_request(&post).unwrap();
        assert_eq!((req.method, req.path, req.body), ("POST", "/v1/join/7", &b"v=0\r\n"[..]));
        // BDS's probe answer, byte for byte (vanilla-capture-2026-10-01).
        let ok = response(200, "application/json", "");
        assert_eq!(ok, b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
        assert!(response_complete(&ok));
    }

    #[test]
    fn response_completes_at_content_length() {
        assert!(!response_complete(b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\n\r\nv="));
        assert!(response_complete(b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\n\r\nv=0\n"));
        assert!(!response_complete(b"HTTP/1.1 200 OK\r\n\r\nno length"));
    }
}
