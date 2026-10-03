use std::fmt;

/// A SOCKS5 proxy that must support UDP ASSOCIATE (Bedrock is UDP-only).
#[derive(Clone, PartialEq, Eq)]
pub struct Socks5Proxy {
    pub host: String,
    pub port: u16,
    pub credentials: Option<(String, String)>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("invalid proxy {0:?}: expected host:port, host:port:user:pass or socks5://[user:pass@]host:port")]
pub struct ProxyParseError(String);

impl Socks5Proxy {
    pub fn new(host: impl Into<String>, port: u16) -> Self {
        Self { host: host.into(), port, credentials: None }
    }

    pub fn login(mut self, user: impl Into<String>, pass: impl Into<String>) -> Self {
        self.credentials = Some((user.into(), pass.into()));
        self
    }

    /// Accepts the provider export format `host:port[:user:pass]` or a `socks5://` URL.
    /// Passwords may contain `:` in the colon format (everything after the third colon).
    pub fn parse(s: &str) -> Result<Self, ProxyParseError> {
        let err = || ProxyParseError(redact(s));
        let s = s.trim();
        if let Some(rest) = s.strip_prefix("socks5://").or_else(|| s.strip_prefix("socks5h://")) {
            let (userinfo, hostport) = match rest.rsplit_once('@') {
                Some((u, h)) => (Some(u), h),
                None => (None, rest),
            };
            let (host, port) = hostport.trim_end_matches('/').rsplit_once(':').ok_or_else(err)?;
            let mut proxy = Self::new(host.trim_start_matches('[').trim_end_matches(']'), port.parse().map_err(|_| err())?);
            if let Some(u) = userinfo {
                let (user, pass) = u.split_once(':').ok_or_else(err)?;
                proxy = proxy.login(percent_decode(user), percent_decode(pass));
            }
            return Ok(proxy);
        }
        let mut parts = s.splitn(4, ':');
        let (host, port) = (parts.next().ok_or_else(err)?, parts.next().ok_or_else(err)?);
        let proxy = Self::new(host, port.parse().map_err(|_| err())?);
        match (parts.next(), parts.next()) {
            (None, None) => Ok(proxy),
            (Some(user), Some(pass)) if !user.is_empty() => Ok(proxy.login(user, pass)),
            _ => Err(err()),
        }
    }

    /// `socks5h://` URL (DNS resolved by the proxy) for HTTP clients, e.g. `AuthConfig::proxy`, so
    /// sign-in traffic leaves from the same IP as the game connection.
    pub fn to_url(&self) -> String {
        match &self.credentials {
            Some((u, p)) => format!("socks5h://{}:{}@{}:{}", percent_encode(u), percent_encode(p), self.host, self.port),
            None => format!("socks5h://{}:{}", self.host, self.port),
        }
    }
}

impl fmt::Debug for Socks5Proxy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let user = self.credentials.as_ref().map(|(u, _)| u.as_str());
        f.debug_struct("Socks5Proxy").field("host", &self.host).field("port", &self.port).field("user", &user).finish_non_exhaustive()
    }
}

/// Keeps error messages and logs free of the password in `host:port:user:pass` strings.
fn redact(s: &str) -> String {
    let mut parts: Vec<&str> = s.splitn(4, ':').collect();
    if parts.len() == 4 {
        parts[3] = "***";
    }
    parts.join(":")
}

fn percent_encode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        let hex = (b[i] == b'%').then(|| b.get(i + 1..i + 3)).flatten().and_then(|h| u8::from_str_radix(std::str::from_utf8(h).ok()?, 16).ok());
        match hex {
            Some(v) => {
                out.push(v);
                i += 3;
            }
            None => {
                out.push(b[i]);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_provider_and_url_formats() {
        let p = Socks5Proxy::parse("isp.example.com:10001:user1:pa:ss~w0rd").unwrap();
        assert_eq!((p.host.as_str(), p.port), ("isp.example.com", 10001));
        assert_eq!(p.credentials, Some(("user1".into(), "pa:ss~w0rd".into())));
        assert_eq!(Socks5Proxy::parse("1.2.3.4:1080").unwrap().credentials, None);
        let u = Socks5Proxy::parse("socks5://us%40er:p%3Ass@host.net:9050").unwrap();
        assert_eq!(u.credentials, Some(("us@er".into(), "p:ss".into())));
        assert_eq!(Socks5Proxy::parse(&u.to_url()).unwrap(), u);
        assert!(Socks5Proxy::parse("host-only").is_err());
    }

    #[test]
    fn password_never_appears_in_debug_or_errors() {
        let p = Socks5Proxy::parse("h:1:user:hunter2").unwrap();
        assert!(!format!("{p:?}").contains("hunter2"));
        let e = Socks5Proxy::parse("h:notaport:user:hunter2").unwrap_err();
        assert!(!e.to_string().contains("hunter2"));
    }
}
