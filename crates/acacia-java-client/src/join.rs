use acacia_auth::{Account, JavaProfile};
use acacia_java_proto::packets::{AcceptTeleportation, Empty, EncryptionHello, EncryptionKey, INTENT_LOGIN, Intention, KeepAlive, LoginCompression, LoginFinished, LoginHello, Ping, PlayerPosition, SelectKnownPacks};
use acacia_java_proto::recipes::{StonecutterRecipe, read_update_recipes};
use acacia_java_proto::{PROTOCOL_VERSION, VarInt, Wire, ids, read_varint};
use md5::{Digest, Md5};
use tokio::net::TcpStream;

use crate::actions::Actions;
use crate::crypto::{random_secret, rsa_encrypt, server_hash};
use crate::frame::{Connection, Error};
use crate::inventory::Inventory;
use crate::link::{Link, Packet};
use crate::movement::Movement;
use crate::world::World;

/// The id an offline-mode server gives `name`: a v3 UUID of `OfflinePlayer:<name>`.
pub fn offline_uuid(name: &str) -> u128 {
    let mut hash: [u8; 16] = Md5::digest(format!("OfflinePlayer:{name}")).into();
    hash[6] = hash[6] & 0x0f | 0x30;
    hash[8] = hash[8] & 0x3f | 0x80;
    u128::from_be_bytes(hash)
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
    link: Link,
    pub uuid: u128,
    pub name: String,
    pub pose: Pose,
    pub world: World,
    pub movement: Movement,
    pub actions: Actions,
    pub inventory: Inventory,
    /// The stonecutter's recipes in the server's order (its button order), from the last `UpdateRecipes`.
    pub stonecutter: Vec<StonecutterRecipe>,
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
        let enchantments = configure(&mut c).await?;
        let mut inventory = Inventory::default();
        inventory.enchantments = enchantments.into();
        Ok(Self { link: Link::spawn(c), uuid: profile.uuid, name: profile.name, pose: Pose::default(), world: World::default(), movement: Movement::default(), actions: Actions::default(), inventory, stonecutter: Vec::new(), loaded_sent: false })
    }

    /// The next packet, already applied to the world and movement. Cancel-safe.
    pub async fn next(&mut self) -> Result<Packet, Error> {
        let packet = self.link.recv().await.ok_or(Error::Closed)??;
        self.apply(&packet)?;
        Ok(packet)
    }

    /// Like [`Client::next`] for a packet that has already arrived.
    pub fn try_next(&mut self) -> Option<Result<Packet, Error>> {
        let packet = match self.link.try_recv()? {
            Ok(packet) => packet,
            Err(e) => return Some(Err(e)),
        };
        Some(self.apply(&packet).map(|()| packet))
    }

    /// Queues a play packet; [`Error::Closed`] once the connection is gone.
    pub fn send(&self, id: i32, body: &impl Wire) -> Result<(), Error> {
        self.link.send(id, body).then_some(()).ok_or(Error::Closed)
    }

    /// Closes the connection; [`Client::next`] then fails with [`Error::Closed`].
    pub fn close(&self) {
        self.link.close();
    }

    fn apply(&mut self, packet: &Packet) -> Result<(), Error> {
        use ids::play::{clientbound as from, serverbound as to};
        let (id, body) = (packet.id, &packet.body[..]);
        match id {
            from::PING => self.send(to::PONG, &Ping::read(&mut &body[..])?)?,
            from::PLAYER_POSITION => {
                let position = PlayerPosition::read(&mut &body[..])?;
                self.pose = self.pose.teleported(&position);
                self.movement.teleported(self.pose, &position);
                let Pose { position: [x, y, z], y_rot, x_rot } = self.pose;
                self.send(to::ACCEPT_TELEPORTATION, &AcceptTeleportation { id: position.id, x, y, z, y_rot, x_rot })?;
                if !std::mem::replace(&mut self.loaded_sent, true) {
                    self.send(to::PLAYER_LOADED, &Empty {})?;
                }
            }
            from::UPDATE_RECIPES => self.stonecutter = read_update_recipes(&mut &body[..])?,
            from::DISCONNECT => return Err(Error::Disconnected(format!("{body:02x?}"))),
            _ => {}
        }
        self.world.apply(id, body)?;
        self.movement.apply(id, body, self.world.player_id)?;
        self.actions.apply(id, body)?;
        self.inventory.apply(id, body)?;
        Ok(())
    }

    /// The server has placed the player: its first position arrived.
    pub fn loaded(&self) -> bool {
        self.loaded_sent
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

/// The configuration phase; returns the enchantment registry's entry names by network id.
async fn configure(c: &mut Connection) -> Result<Vec<String>, Error> {
    use ids::configuration::{clientbound as from, serverbound as to};
    let mut enchantments = Vec::new();
    loop {
        let (id, body) = c.read().await?;
        let mut input = &body[..];
        match id {
            from::KEEP_ALIVE => c.send(to::KEEP_ALIVE, &KeepAlive::read(&mut input)?).await?,
            from::PING => c.send(to::PONG, &Ping::read(&mut input)?).await?,
            // Claiming the server's own packs means it omits the vanilla registry contents.
            from::SELECT_KNOWN_PACKS => c.send(to::SELECT_KNOWN_PACKS, &SelectKnownPacks::read(&mut input)?).await?,
            from::REGISTRY_DATA => {
                if String::read(&mut input)? == ENCHANTMENT_REGISTRY {
                    enchantments = registry_entries(&mut input)?;
                }
            }
            from::FINISH_CONFIGURATION => {
                c.send(to::FINISH_CONFIGURATION, &Empty {}).await?;
                return Ok(enchantments);
            }
            from::DISCONNECT => return Err(Error::Disconnected(format!("{body:02x?}"))),
            _ => {}
        }
    }
}

const ENCHANTMENT_REGISTRY: &str = "minecraft:enchantment";

/// `RegistryData` entries after the registry key: ids in network order, each with optional data (absent for packs
/// both sides know).
fn registry_entries(input: &mut &[u8]) -> Result<Vec<String>, Error> {
    let mut names = Vec::new();
    for _ in 0..read_varint(input)? {
        names.push(String::read(input)?);
        if bool::read(input)? {
            acacia_java_proto::nbt::skip_tag(input)?;
        }
    }
    Ok(names)
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
