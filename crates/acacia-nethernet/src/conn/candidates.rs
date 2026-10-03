use std::net::SocketAddr;
use std::time::Instant;

use str0m::Candidate;

use super::Connection;
use crate::sdp::{self, CandidateType};
use crate::Error;

/// A gathered local candidate to trickle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalCandidate {
    /// The address to advertise for the socket (see `Transport::local_candidate` in acacia-client).
    Host(SocketAddr),
    /// `addr` is how a STUN server saw `base`, the host candidate.
    ServerReflexive { addr: SocketAddr, base: SocketAddr },
    /// A TURN allocation at `addr`; `mapped` is our address as the TURN server saw it.
    Relayed { addr: SocketAddr, mapped: SocketAddr },
}

impl Connection {
    /// Adds a local candidate and returns its `candidate:` line for `CANDIDATEADD`.
    pub fn add_local_candidate(&mut self, candidate: LocalCandidate, now: Instant) -> Result<String, Error> {
        fn bad(e: impl std::fmt::Display) -> Error {
            Error::Sdp(e.to_string())
        }
        let (c, addr, typ) = match candidate {
            LocalCandidate::Host(addr) => {
                if self.host.is_some_and(|h| h != addr) {
                    return Err(Error::Sdp("only one host candidate is supported".into()));
                }
                self.host = Some(addr);
                (Candidate::host(addr, "udp").map_err(bad)?, addr, CandidateType::Host)
            }
            LocalCandidate::ServerReflexive { addr, base } => (
                Candidate::server_reflexive(addr, base, "udp").map_err(bad)?,
                addr,
                CandidateType::ServerReflexive(base),
            ),
            LocalCandidate::Relayed { addr, mapped } => {
                self.relays.push(addr);
                (Candidate::relayed(addr, mapped, "udp").map_err(bad)?, addr, CandidateType::Relay(mapped))
            }
        };
        // str0m drops candidates it finds redundant (srflx sharing a host base); vanilla still trickles them.
        if self.rtc.add_local_candidate(c).is_none() {
            tracing::debug!(%addr, ?typ, "str0m ignored local candidate");
        }
        self.drive(now);
        Ok(sdp::candidate_line(addr, typ, Some(&self.ufrag)))
    }

    /// Adds a trickled remote `candidate:` line (an `a=` prefix is tolerated).
    pub fn add_remote_candidate(&mut self, line: &str, now: Instant) -> Result<(), Error> {
        let line = line.trim();
        let line = line.strip_prefix("a=").unwrap_or(line);
        let c = Candidate::from_sdp_string(line).map_err(|e| Error::Sdp(e.to_string()))?;
        if self.pending_offer.is_some() {
            self.early_candidates.push(c);
        } else {
            self.rtc.add_remote_candidate(c);
            self.drive(now);
        }
        Ok(())
    }
}
