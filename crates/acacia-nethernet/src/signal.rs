//! Signal text shared by LAN discovery and the signaling service: `<TYPE> <connection id> <data>`.
//! Spec: docs/research/nethernet-signaling.md §1d.

use std::fmt;

use crate::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignalKind {
    /// Data is the SDP offer.
    ConnectRequest,
    /// Data is the SDP answer.
    ConnectResponse,
    /// Data is one `candidate:…` line.
    CandidateAdd,
    /// Data is a decimal error code (see [`crate::signaling_error_name`]).
    ConnectError,
}

impl SignalKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ConnectRequest => "CONNECTREQUEST",
            Self::ConnectResponse => "CONNECTRESPONSE",
            Self::CandidateAdd => "CANDIDATEADD",
            Self::ConnectError => "CONNECTERROR",
        }
    }

    fn parse(s: &str) -> Option<Self> {
        [Self::ConnectRequest, Self::ConnectResponse, Self::CandidateAdd, Self::ConnectError]
            .into_iter()
            .find(|k| k.as_str() == s)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Signal {
    pub kind: SignalKind,
    pub connection_id: u64,
    pub data: String,
}

impl Signal {
    pub fn new(kind: SignalKind, connection_id: u64, data: impl Into<String>) -> Self {
        Self { kind, connection_id, data: data.into() }
    }

    pub fn error(connection_id: u64, code: u32) -> Self {
        Self::new(SignalKind::ConnectError, connection_id, code.to_string())
    }

    pub fn parse(text: &str) -> Result<Self, Error> {
        let bad = |why: &str| Error::Signal(format!("{why}: {:?}", truncate(text)));
        let mut parts = text.splitn(3, ' ');
        let kind = parts.next().and_then(SignalKind::parse).ok_or_else(|| bad("unknown type"))?;
        let id = parts.next().ok_or_else(|| bad("no connection id"))?;
        // go-nethernet reads the id as a decimal prefix, so `123abc` is connection 123.
        let digits = id.bytes().take_while(u8::is_ascii_digit).count();
        let connection_id = id[..digits].parse().map_err(|_| bad("bad connection id"))?;
        Ok(Self { kind, connection_id, data: parts.next().unwrap_or_default().to_owned() })
    }

    /// The code a `CONNECTERROR` carries.
    pub fn error_code(&self) -> Option<u32> {
        (self.kind == SignalKind::ConnectError).then(|| self.data.trim().parse().ok()).flatten()
    }
}

impl fmt::Display for Signal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {} {}", self.kind.as_str(), self.connection_id, self.data)
    }
}

fn truncate(s: &str) -> &str {
    s.char_indices().nth(64).map_or(s, |(i, _)| &s[..i])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_sdp_with_spaces_and_newlines() {
        let s = Signal::new(SignalKind::ConnectRequest, 18446744073709551615, "v=0\r\no=- 1 2 IN IP4 127.0.0.1\r\n");
        assert_eq!(Signal::parse(&s.to_string()).unwrap(), s);
        assert!(s.to_string().starts_with("CONNECTREQUEST 18446744073709551615 v=0\r\n"));
    }

    #[test]
    fn parses_candidates_and_errors() {
        let c = Signal::parse("CANDIDATEADD 7 candidate:1 1 udp 2113937151 192.168.1.2 5000 typ host").unwrap();
        assert_eq!((c.kind, c.connection_id), (SignalKind::CandidateAdd, 7));
        assert_eq!(c.data, "candidate:1 1 udp 2113937151 192.168.1.2 5000 typ host");
        let e = Signal::parse("CONNECTERROR 7 37").unwrap();
        assert_eq!(e.error_code(), Some(37));
        assert_eq!(Signal::error(7, 37), e);
        assert_eq!(c.error_code(), None);
    }

    #[test]
    fn rejects_unknown_type_and_missing_id() {
        assert!(Signal::parse("HELLO 1 x").is_err());
        assert!(Signal::parse("CONNECTREQUEST").is_err());
        assert!(Signal::parse("CONNECTREQUEST abc x").is_err());
        assert_eq!(Signal::parse("CONNECTERROR 12x 3").unwrap().connection_id, 12);
        assert_eq!(Signal::parse("CONNECTRESPONSE 5").unwrap().data, "");
    }
}
