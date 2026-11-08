use acacia_auth::{Account, JavaProfile};
use acacia_java_proto::packets::{AcceptTeleportation, ChunkBatchReceived, Empty, EncryptionHello, EncryptionKey, INTENT_LOGIN, Intention, KeepAlive, LoginCompression, LoginFinished, LoginHello, Ping, PlayerPosition, SelectKnownPacks};
use acacia_java_proto::{PROTOCOL_VERSION, VarInt, Wire, ids};
use md5::{Digest, Md5};
use tokio::net::TcpStream;

use crate::crypto::{random_secret, rsa_encrypt, server_hash};
use crate::frame::{Connection, Error};
use crate::world::World;

/// Chunks per tick the client says it can take, as a client with time to spare does.
const CHUNKS_PER_TICK: f32 = 20.0;

/// The id an offline-mode server gives `name`: a v3 UUID of `OfflinePlayer:<name>`.
pub fn offline_uuid(name: &str) -> u128 {
    let mut hash: [u8; 16] = Md5::digest(format!("OfflinePlayer:{name}")).into();
    hash[6] = hash[6] & 0x0f | 0x30;
    hash[8] = hash[8] & 0x3f | 0x80;
    u128::from_be_bytes(hash)
}

/// A packet in the play state that [`Client::next`] did not answer by itself.
#[derive(Debug)]
pub struct Packet {
    pub id: i32,
    pub body: Vec<u8>,
}

/// Where the server last put the player.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Pose {
    pub position: [f64; 3],
    pub y_rot: f32,
    pub x_rot: f32,
}

impl Pose {
    /// `PositionMoveRotation.calculateAbsolute`; `relatives` holds `Relative` bits (X, Y, Z, Y_ROT, X_ROT from bit 0).
    fn teleported(self, packet: &PlayerPosition) -> Pose {
        let c = &packet.change;
        let is = |bit: u32| packet.relatives & (1 << bit) != 0;
        let base = |bit, value: f64| if is(bit) { value } else { 0.0 };
        let [x, y, z] = self.position;
        Pose {
            position: [base(0, x) + c.x, base(1, y) + c.y, base(2, z) + c.z],
            y_rot: if is(3) { self.y_rot } else { 0.0 } + c.y_rot,
            x_rot: (if is(4) { self.x_rot } else { 0.0 } + c.x_rot).clamp(-90.0, 90.0),
        }
    }
}

/// A client in the play state.
pub struct Client {
    connection: Connection,
    pub uuid: u128,
    pub name: String,
    pub pose: Pose,
    pub world: World,
    loaded_sent: bool,
}

impl Client {
    /// Joins an offline-mode server as `name`.
    pub async fn join_offline(host: &str, port: u16, name: &str) -> Result<Self, Error> {
        Self::join(host, port, name, offline_uuid(name), None).await
    }

    /// Joins any server as the account's Java profile.
    pub async fn join_online(host: &str, port: u16, account: &Account) -> Result<Self, Error> {
        let player = account.java_profile().await?;
        let uuid = u128::from_str_radix(&player.uuid, 16).map_err(|e| Error::Crypto(format!("profile id {}: {e}", player.uuid)))?;
        Self::join(host, port, &player.name.clone(), uuid, Some((account, &player))).await
    }

    async fn join(host: &str, port: u16, name: &str, uuid: u128, session: Session<'_>) -> Result<Self, Error> {
        let mut c = Connection::new(TcpStream::connect((host, port)).await?);
        let intention = Intention { protocol: VarInt(PROTOCOL_VERSION), host: host.to_owned(), port, intent: VarInt(INTENT_LOGIN) };
        c.send(ids::handshake::serverbound::INTENTION, &intention).await?;
        c.send(ids::login::serverbound::HELLO, &LoginHello { name: name.to_owned(), uuid }).await?;
        let profile = login(&mut c, session).await?;
        configure(&mut c).await?;
        Ok(Self { connection: c, uuid: profile.uuid, name: profile.name, pose: Pose::default(), world: World::default(), loaded_sent: false })
    }

    /// The next packet, after sending any reply the protocol requires for it.
    pub async fn next(&mut self) -> Result<Packet, Error> {
        use ids::play::{clientbound as from, serverbound as to};
        let (id, body) = self.connection.read().await?;
        let mut input = &body[..];
        match id {
            from::KEEP_ALIVE => self.connection.send(to::KEEP_ALIVE, &KeepAlive::read(&mut input)?).await?,
            from::PING => self.connection.send(to::PONG, &Ping::read(&mut input)?).await?,
            from::CHUNK_BATCH_FINISHED => self.connection.send(to::CHUNK_BATCH_RECEIVED, &ChunkBatchReceived { chunks_per_tick: CHUNKS_PER_TICK }).await?,
            from::PLAYER_POSITION => {
                let packet = PlayerPosition::read(&mut input)?;
                self.pose = self.pose.teleported(&packet);
                let Pose { position: [x, y, z], y_rot, x_rot } = self.pose;
                self.connection.send(to::ACCEPT_TELEPORTATION, &AcceptTeleportation { id: packet.id, x, y, z, y_rot, x_rot }).await?;
                if !std::mem::replace(&mut self.loaded_sent, true) {
                    self.connection.send(to::PLAYER_LOADED, &Empty {}).await?;
                }
            }
            from::DISCONNECT => return Err(Error::Disconnected(format!("{body:02x?}"))),
            _ => {}
        }
        self.world.apply(id, &body)?;
        Ok(Packet { id, body })
    }
}

/// The account and profile to authenticate with; `None` joins offline.
type Session<'a> = Option<(&'a Account, &'a JavaProfile)>;

/// Answers the server's encryption request and turns encryption on.
async fn encrypt(c: &mut Connection, hello: EncryptionHello, session: Session<'_>) -> Result<(), Error> {
    let secret = random_secret();
    if hello.should_authenticate {
        let (account, player) = session.ok_or(Error::NeedsAccount)?;
        account.join_java_session(player, &server_hash(&hello.server_id, &secret, &hello.public_key)).await?;
    }
    let key = EncryptionKey { secret: rsa_encrypt(&hello.public_key, &secret)?, challenge: rsa_encrypt(&hello.public_key, &hello.challenge)? };
    c.send(ids::login::serverbound::KEY, &key).await?;
    c.enable_encryption(&secret);
    Ok(())
}

async fn login(c: &mut Connection, session: Session<'_>) -> Result<LoginFinished, Error> {
    use ids::login::{clientbound as from, serverbound as to};
    loop {
        let (id, body) = c.read().await?;
        let mut input = &body[..];
        match id {
            from::HELLO => encrypt(c, EncryptionHello::read(&mut input)?, session).await?,
            from::LOGIN_COMPRESSION => c.set_compression(LoginCompression::read(&mut input)?.threshold.0),
            from::LOGIN_FINISHED => {
                let profile = LoginFinished::read(&mut input)?;
                c.send(to::LOGIN_ACKNOWLEDGED, &Empty {}).await?;
                return Ok(profile);
            }
            from::LOGIN_DISCONNECT => return Err(Error::Disconnected(String::read(&mut input)?)),
            other => return Err(Error::Unexpected(other, "login")),
        }
    }
}

async fn configure(c: &mut Connection) -> Result<(), Error> {
    use ids::configuration::{clientbound as from, serverbound as to};
    loop {
        let (id, body) = c.read().await?;
        let mut input = &body[..];
        match id {
            from::KEEP_ALIVE => c.send(to::KEEP_ALIVE, &KeepAlive::read(&mut input)?).await?,
            from::PING => c.send(to::PONG, &Ping::read(&mut input)?).await?,
            // Claiming the server's own packs means it omits the vanilla registry contents.
            from::SELECT_KNOWN_PACKS => c.send(to::SELECT_KNOWN_PACKS, &SelectKnownPacks::read(&mut input)?).await?,
            from::FINISH_CONFIGURATION => {
                c.send(to::FINISH_CONFIGURATION, &Empty {}).await?;
                return Ok(());
            }
            from::DISCONNECT => return Err(Error::Disconnected(format!("{body:02x?}"))),
            _ => {}
        }
    }
}

#[cfg(test)]
#[path = "join_tests.rs"]
mod join_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offline_uuid_matches_vanilla() {
        // UUIDUtil.createOfflinePlayerUUID("Notch")
        assert_eq!(offline_uuid("Notch"), 0xb50ad385_829d_3141_a216_7e7d7539ba7f);
    }

    #[test]
    fn teleports_apply_relative_bits() {
        let start = Pose { position: [10.0, 64.0, -3.0], y_rot: 90.0, x_rot: 80.0 };
        let change = acacia_java_proto::packets::PositionMoveRotation { x: 1.0, y: 2.0, z: 3.0, y_rot: 5.0, x_rot: 20.0, ..Default::default() };
        let packet = |relatives| PlayerPosition { change: change.clone(), relatives, ..Default::default() };
        assert_eq!(start.teleported(&packet(0)), Pose { position: [1.0, 2.0, 3.0], y_rot: 5.0, x_rot: 20.0 });
        assert_eq!(start.teleported(&packet(0b11001)), Pose { position: [11.0, 2.0, 3.0], y_rot: 95.0, x_rot: 90.0 });
    }
}
