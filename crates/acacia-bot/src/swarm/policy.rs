use std::sync::Arc;
use std::time::Duration;

use acacia_client::auth::Error as AuthError;
use acacia_client::proto::packets::PlayStatusStatus;
use acacia_client::{ConnectError, DisconnectReason};

use super::spec::BotId;

/// What to do after a session ends; [`Policy::on_disconnect`] may replace the default.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Decision {
    Reconnect,
    /// Join this `host:port` next (a server transfer); later reconnects go back to the spec's target.
    Follow { address: String },
    Stop { error: String },
}

pub type DisconnectHook = Arc<dyn Fn(&BotId, &DisconnectReason, Decision) -> Decision + Send + Sync>;

#[derive(Clone)]
pub struct Policy {
    /// Waits double from `initial` up to `max`, ±25 %; a session of `healthy_after` resets them.
    pub initial: Duration,
    pub max: Duration,
    pub healthy_after: Duration,
    /// Give up after `kick_limit` kicks in a row that each came within `kick_window` of spawning
    /// (a ban or whitelist, not a restart).
    pub kick_limit: u32,
    pub kick_window: Duration,
    pub on_disconnect: Option<DisconnectHook>,
}

impl Default for Policy {
    fn default() -> Self {
        Self {
            initial: Duration::from_secs(1),
            max: Duration::from_secs(60),
            healthy_after: Duration::from_secs(60),
            kick_limit: 3,
            kick_window: Duration::from_secs(30),
            on_disconnect: None,
        }
    }
}

pub(crate) enum Ended<'a> {
    Session { reason: &'a DisconnectReason, uptime: Duration },
    Connect(&'a ConnectError),
    Auth(&'a AuthError),
    /// The spec cannot work as given (bad proxy, realm with an offline login).
    Invalid(String),
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Next {
    After(Duration),
    Follow(String),
    Stop(String),
}

/// Per-bot retry bookkeeping across reconnects.
#[derive(Default)]
pub(crate) struct Retry {
    failures: u32,
    quick_kicks: u32,
}

impl Policy {
    pub(crate) fn next(&self, id: &BotId, retry: &mut Retry, ended: Ended<'_>, noise: u64) -> Next {
        let decision = match ended {
            Ended::Session { reason, uptime } => {
                if uptime >= self.healthy_after {
                    retry.failures = 0;
                }
                let default = self.session_decision(retry, reason, uptime);
                self.hooked(id, reason, default)
            }
            Ended::Connect(ConnectError::Disconnected(reason)) => {
                let default = self.session_decision(retry, reason, Duration::ZERO);
                self.hooked(id, reason, default)
            }
            Ended::Connect(e @ (ConnectError::NetherNetNeedsOnline | ConnectError::NetherNetUnsupported)) => stop(e),
            Ended::Connect(_) => Decision::Reconnect,
            Ended::Auth(e) if permanent(e) => stop(e),
            Ended::Auth(_) => Decision::Reconnect,
            Ended::Invalid(error) => Decision::Stop { error },
        };
        match decision {
            Decision::Reconnect => {
                let delay = self.delay(retry.failures, noise);
                retry.failures += 1;
                Next::After(delay)
            }
            Decision::Follow { address } => Next::Follow(address),
            Decision::Stop { error } => Next::Stop(error),
        }
    }

    fn hooked(&self, id: &BotId, reason: &DisconnectReason, default: Decision) -> Decision {
        match &self.on_disconnect {
            Some(hook) => hook(id, reason, default),
            None => default,
        }
    }

    fn session_decision(&self, retry: &mut Retry, reason: &DisconnectReason, uptime: Duration) -> Decision {
        match reason {
            DisconnectReason::Kicked { message, .. } if uptime < self.kick_window => {
                retry.quick_kicks += 1;
                if retry.quick_kicks >= self.kick_limit {
                    let error = format!("kicked {} times within {:?} of joining: {message}", retry.quick_kicks, self.kick_window);
                    return Decision::Stop { error };
                }
                Decision::Reconnect
            }
            DisconnectReason::Transfer { address, port } => Decision::Follow { address: host_port(address, *port) },
            DisconnectReason::LoginFailed(status) if *status != PlayStatusStatus::FailedServerFull => {
                Decision::Stop { error: format!("login refused: {status:?}") }
            }
            _ => {
                retry.quick_kicks = 0;
                Decision::Reconnect
            }
        }
    }

    fn delay(&self, failures: u32, noise: u64) -> Duration {
        let base = self.initial.saturating_mul(1 << failures.min(16)).min(self.max);
        base * (75 + (noise % 51) as u32) / 100
    }
}

/// Sign-in problems only the account holder can fix.
fn permanent(e: &AuthError) -> bool {
    e.requires_user_action() || matches!(e, AuthError::OAuth { .. } | AuthError::DeviceCodeExpired | AuthError::DeviceCodeDeclined)
}

fn stop(e: impl std::fmt::Display) -> Decision {
    Decision::Stop { error: e.to_string() }
}

fn host_port(host: &str, port: u16) -> String {
    if host.contains(':') { format!("[{host}]:{port}") } else { format!("{host}:{port}") }
}

#[cfg(test)]
mod tests;
