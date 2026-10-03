//! ICE servers from the signaling service's TURN credentials (docs/research/nethernet-signaling.md §2e).

use std::str::FromStr;
use std::time::Duration;

use serde_json::Value;

use super::TurnError;

/// `{"ExpirationInSeconds","TurnAuthServers":[{"Username","Password","Urls":[…]}]}`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IceServers {
    /// How long the credentials stay valid; cache them for this long.
    pub expires_in: Duration,
    pub servers: Vec<IceServer>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IceServer {
    pub username: String,
    pub password: String,
    pub urls: Vec<IceUrl>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scheme {
    Stun,
    Turn,
    Turns,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transport {
    Udp,
    Tcp,
}

/// A `stun:`/`turn:`/`turns:` URL (RFC 7064/7065). The host is left unresolved for the driver.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IceUrl {
    pub scheme: Scheme,
    pub host: String,
    pub port: u16,
    pub transport: Transport,
}

impl IceServers {
    pub fn parse(json: &str) -> Result<Self, TurnError> {
        Self::from_json(&serde_json::from_str(json).map_err(|e| TurnError::Config(e.to_string()))?)
    }

    pub fn from_json(root: &Value) -> Result<Self, TurnError> {
        let bad = |what: &str| TurnError::Config(format!("missing or bad {what}"));
        let expires_in = root["ExpirationInSeconds"].as_u64().ok_or_else(|| bad("ExpirationInSeconds"))?;
        let servers = root["TurnAuthServers"].as_array().ok_or_else(|| bad("TurnAuthServers"))?;
        let servers = servers
            .iter()
            .map(|s| {
                let text = |key: &str| s[key].as_str().map(str::to_owned).ok_or_else(|| bad(key));
                let urls = s["Urls"].as_array().ok_or_else(|| bad("Urls"))?;
                Ok(IceServer {
                    username: text("Username")?,
                    password: text("Password")?,
                    urls: urls.iter().map(|u| u.as_str().ok_or_else(|| bad("Urls"))?.parse()).collect::<Result<_, _>>()?,
                })
            })
            .collect::<Result<_, TurnError>>()?;
        Ok(Self { expires_in: Duration::from_secs(expires_in), servers })
    }

    /// Every `(url, server)` pair, for picking STUN and UDP TURN endpoints.
    pub fn urls(&self) -> impl Iterator<Item = (&IceUrl, &IceServer)> {
        self.servers.iter().flat_map(|s| s.urls.iter().map(move |u| (u, s)))
    }
}

impl FromStr for IceUrl {
    type Err = TurnError;

    fn from_str(url: &str) -> Result<Self, TurnError> {
        let bad = || TurnError::Config(format!("bad ICE server URL {url:?}"));
        let (scheme, rest) = url.split_once(':').ok_or_else(bad)?;
        let scheme = match scheme.to_ascii_lowercase().as_str() {
            "stun" => Scheme::Stun,
            "turn" => Scheme::Turn,
            "turns" => Scheme::Turns,
            _ => return Err(bad()),
        };
        let (authority, query) = rest.split_once('?').unwrap_or((rest, ""));
        let (host, port) = match authority.strip_prefix('[') {
            Some(v6) => {
                let (host, after) = v6.split_once(']').ok_or_else(bad)?;
                (host, after.strip_prefix(':'))
            }
            None => match authority.rsplit_once(':') {
                Some((host, port)) => (host, Some(port)),
                None => (authority, None),
            },
        };
        let tls = scheme == Scheme::Turns;
        let port = match port {
            Some(p) => p.parse().map_err(|_| bad())?,
            None if tls => 5349,
            None => 3478,
        };
        let transport = match query.strip_prefix("transport=") {
            Some(t) if t.eq_ignore_ascii_case("udp") => Transport::Udp,
            Some(t) if t.eq_ignore_ascii_case("tcp") => Transport::Tcp,
            Some(_) => return Err(bad()),
            None if tls => Transport::Tcp,
            None => Transport::Udp,
        };
        if host.is_empty() {
            return Err(bad());
        }
        Ok(Self { scheme, host: host.to_owned(), port, transport })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_turn_auth_servers() {
        let json = r#"{"ExpirationInSeconds":86400,"TurnAuthServers":[{"Username":"u1","Password":"p1",
            "Urls":["stun:relay.example.net:3478","turn:relay.example.net:3478?transport=udp","turns:[2001:db8::1]"]}]}"#;
        let ice = IceServers::parse(json).unwrap();
        assert_eq!(ice.expires_in, Duration::from_secs(86400));
        let s = &ice.servers[0];
        assert_eq!((s.username.as_str(), s.password.as_str()), ("u1", "p1"));
        let url = |scheme, host: &str, port, transport| IceUrl { scheme, host: host.into(), port, transport };
        assert_eq!(s.urls, [
            url(Scheme::Stun, "relay.example.net", 3478, Transport::Udp),
            url(Scheme::Turn, "relay.example.net", 3478, Transport::Udp),
            url(Scheme::Turns, "2001:db8::1", 5349, Transport::Tcp),
        ]);
        assert_eq!(ice.urls().count(), 3);
    }

    #[test]
    fn rejects_bad_input() {
        for url in ["http://x", "turn:", "turn:h:port", "turn:h?transport=sctp", "turn:[::1"] {
            assert!(url.parse::<IceUrl>().is_err(), "{url}");
        }
        assert_eq!("TURN:h?transport=TCP".parse::<IceUrl>().unwrap().transport, Transport::Tcp);
        assert!(IceServers::parse(r#"{"TurnAuthServers":[]}"#).is_err());
        assert!(IceServers::parse(r#"{"ExpirationInSeconds":1,"TurnAuthServers":[{"Username":"u"}]}"#).is_err());
    }
}
