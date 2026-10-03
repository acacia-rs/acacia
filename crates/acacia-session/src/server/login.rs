//! The login sequence as a server runs it: RequestNetworkSettings → NetworkSettings, Login →
//! (verified) ServerToClientHandshake, ClientToServerHandshake → PlayStatus(LoginSuccess). Everything
//! after that (resource packs, StartGame) is the caller's. Deadlines are the caller's too.

use acacia_auth::login::verify::{VerifiedLogin, Verifier, VerifyError};
use acacia_proto::packets::{
    ClientToServerHandshake, Disconnect, DisconnectContent, Login, NetworkSettings, NetworkSettingsCompressionAlgorithm, PlayStatus,
    PlayStatusStatus, RequestNetworkSettings,
};
use acacia_proto::types::DisconnectFailReason;
use acacia_proto::{codec, encode_packet, Packet, RawPacket, PROTOCOL_VERSION};
use bytes::{Bytes, BytesMut};

use super::ServerConnection;
use crate::Error;

#[derive(Debug, Clone)]
pub struct LoginConfig {
    /// The only protocol version admitted.
    pub protocol: i32,
    pub compression: NetworkSettingsCompressionAlgorithm,
    /// Batches at least this long are compressed (BDS default 1).
    pub compression_threshold: u16,
    /// Reject logins nobody vouches for (`online-mode`).
    pub require_authenticated: bool,
}

impl Default for LoginConfig {
    fn default() -> Self {
        Self {
            protocol: PROTOCOL_VERSION,
            compression: NetworkSettingsCompressionAlgorithm::Deflate,
            compression_threshold: 1,
            require_authenticated: true,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum LoginError {
    #[error("client protocol {client}, server {server}")]
    Protocol { client: i32, server: i32 },
    #[error("login not verified: {0}")]
    Verify(#[from] VerifyError),
    #[error("login is not authenticated")]
    NotAuthenticated,
    #[error("packet {0} out of order during login")]
    Unexpected(u32),
    #[error("malformed login packet: {0}")]
    Malformed(String),
    #[error(transparent)]
    Session(#[from] Error),
}

/// A failed login: `send` holds the game message telling the client why, to send before closing.
#[derive(Debug)]
pub struct Rejected {
    pub error: LoginError,
    pub send: Option<Bytes>,
}

/// What one client message led to: game messages to send, and the login once it has completed.
#[derive(Debug, Default)]
pub struct Step {
    pub send: Vec<Bytes>,
    pub done: Option<Box<VerifiedLogin>>,
}

enum State {
    Settings,
    Login,
    Handshake(Box<VerifiedLogin>),
    Done,
}

pub struct ServerLogin {
    conn: ServerConnection,
    config: LoginConfig,
    state: State,
}

impl ServerLogin {
    pub fn new(conn: ServerConnection, config: LoginConfig) -> Self {
        Self { conn, config, state: State::Settings }
    }

    /// The connection, for the rest of the session once [`Step::done`] was returned.
    pub fn into_connection(self) -> ServerConnection {
        self.conn
    }

    /// Feeds one game message from the client. `now` is unix seconds, for token expiry.
    pub fn handle(&mut self, msg: &[u8], verifier: &Verifier, now: i64) -> Result<Step, Rejected> {
        let mut step = Step::default();
        let packets = self.conn.decode(msg).map_err(|e| self.reject(e.into(), None))?;
        for buf in packets {
            let packet = RawPacket::parse(buf).map_err(|e| self.reject(LoginError::Malformed(e.to_string()), None))?;
            self.packet(&packet, verifier, now, &mut step)?;
        }
        Ok(step)
    }

    fn packet(&mut self, packet: &RawPacket, verifier: &Verifier, now: i64, step: &mut Step) -> Result<(), Rejected> {
        match (&self.state, packet.id) {
            (State::Settings, RequestNetworkSettings::ID) => {
                let client = packet.decode::<RequestNetworkSettings>().map_err(|e| self.malformed(e))?.client_protocol;
                self.check_protocol(client)?;
                let settings = NetworkSettings {
                    compression_threshold: self.config.compression_threshold,
                    compression_algorithm: self.config.compression,
                    client_throttle: false,
                    client_throttle_threshold: 0,
                    client_throttle_scalar: 0.0,
                };
                self.conn.start_compression(&settings).map_err(|e| self.reject(e.into(), None))?;
                step.send.push(self.conn.encode(&[encode(&settings)]));
                self.state = State::Login;
            }
            (State::Login, Login::ID) => {
                let (protocol, request) = split_login(&packet.body).map_err(|e| self.reject(e, None))?;
                self.check_protocol(protocol)?;
                let login = verifier.verify(request, now).map_err(|e| {
                    self.reject(e.into(), Some(disconnect(DisconnectFailReason::NotAuthenticated, "disconnectionScreen.notAuthenticated")))
                })?;
                if self.config.require_authenticated && !login.authenticated {
                    let notice = disconnect(DisconnectFailReason::NotAuthenticated, "disconnectionScreen.notAuthenticated");
                    return Err(self.reject(LoginError::NotAuthenticated, Some(notice)));
                }
                // NetherNet stays plaintext after the handshake: DTLS already encrypts (docs/DESIGN.md).
                let handshake =
                    if self.conn.is_nethernet() { self.conn.plaintext_handshake() } else { self.conn.start_encryption(&login.client_key) };
                step.send.push(self.conn.encode(&[handshake]));
                self.state = State::Handshake(Box::new(login));
            }
            (State::Handshake(_), ClientToServerHandshake::ID) => {
                let State::Handshake(login) = std::mem::replace(&mut self.state, State::Done) else { unreachable!() };
                step.send.push(self.conn.encode(&[encode(&PlayStatus { status: PlayStatusStatus::LoginSuccess })]));
                step.done = Some(login);
            }
            (_, id) => return Err(self.reject(LoginError::Unexpected(id), None)),
        }
        Ok(())
    }

    fn check_protocol(&mut self, client: i32) -> Result<(), Rejected> {
        let server = self.config.protocol;
        if client == server {
            return Ok(());
        }
        // FailedClient tells the client it is outdated, FailedSpawn that the server is.
        let status = if client < server { PlayStatusStatus::FailedClient } else { PlayStatusStatus::FailedSpawn };
        Err(self.reject(LoginError::Protocol { client, server }, Some(encode(&PlayStatus { status }))))
    }

    fn malformed(&mut self, e: impl ToString) -> Rejected {
        self.reject(LoginError::Malformed(e.to_string()), None)
    }

    fn reject(&mut self, error: LoginError, notice: Option<Bytes>) -> Rejected {
        self.state = State::Done;
        Rejected { error, send: notice.map(|p| self.conn.encode(&[p])) }
    }
}

/// A Login body: protocol version, then the length-prefixed connection request.
fn split_login(body: &[u8]) -> Result<(i32, &[u8]), LoginError> {
    let mut r = body;
    let malformed = |e: acacia_proto::DecodeError| LoginError::Malformed(e.to_string());
    let protocol = codec::read_i32(&mut r).map_err(malformed)?;
    let len = codec::read_varint(&mut r).map_err(malformed)? as usize;
    let request = r.get(..len).ok_or_else(|| LoginError::Malformed("connection request truncated".into()))?;
    Ok((protocol, request))
}

fn disconnect(reason: DisconnectFailReason, message: &str) -> Bytes {
    let content = DisconnectContent { message: message.into(), filtered_message: String::new() };
    encode(&Disconnect { reason, hide_disconnect_reason: false, content: Some(content) })
}

fn encode<T: Packet>(packet: &T) -> Bytes {
    let mut buf = BytesMut::new();
    encode_packet(packet, &mut buf);
    buf.freeze()
}

#[cfg(test)]
mod tests;
