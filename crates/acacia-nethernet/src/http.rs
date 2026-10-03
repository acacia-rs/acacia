//! The two HTTP/1.1 exchanges of direct-connect signaling, as bytes, with the vanilla client's
//! headers in its order. It asks for keep-alive, so a response ends at Content-Length, not at EOF.

use crate::Error;

/// Signaling responses are capped at 1 MiB (go-nethernet's server limit).
pub const MAX_RESPONSE: usize = 1 << 20;

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

/// True once `raw` holds the headers and the whole Content-Length body (no length: wait for EOF).
pub fn response_complete(raw: &[u8]) -> bool {
    let Some(split) = raw.windows(4).position(|w| w == b"\r\n\r\n") else { return false };
    let head = String::from_utf8_lossy(&raw[..split]).to_ascii_lowercase();
    head.lines()
        .find_map(|l| l.strip_prefix("content-length:"))
        .and_then(|v| v.trim().parse::<usize>().ok())
        .is_some_and(|len| raw.len() >= split + 4 + len)
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
    fn response_completes_at_content_length() {
        assert!(!response_complete(b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\n\r\nv="));
        assert!(response_complete(b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\n\r\nv=0\n"));
        assert!(!response_complete(b"HTTP/1.1 200 OK\r\n\r\nno length"));
    }
}
