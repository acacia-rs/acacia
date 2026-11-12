use acacia_auth::Account;
use acacia_java_proto::dialog::{Dialog, Shown, read_show_dialog};
use acacia_java_proto::packets::{AcceptTeleportation, Empty, INTENT_LOGIN, Intention, LoginHello, Ping, PlayerPosition};
use acacia_java_proto::recipe_book::BookRecipe;
use acacia_java_proto::recipes::{StonecutterRecipe, read_update_recipes};
use acacia_java_proto::{PROTOCOL_VERSION, VarInt, Wire, ids};
use tokio::net::TcpStream;

use std::sync::Mutex;

use crate::actions::Actions;
use crate::chat::ChatState;
use crate::configuration::{Cookies, Registries};
use crate::frame::{Connection, Error};
use crate::inventory::Inventory;
use crate::link::{Link, Packet, Received};
use crate::loading::Loading;
use crate::login::{Session, configure, login};
use crate::messages::{self, Message};
use crate::movement::Movement;
use crate::world::World;

pub use crate::login::offline_uuid;

/// Where the server last put the player.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Pose {
    pub position: [f64; 3],
    pub y_rot: f32,
    pub x_rot: f32,
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
    /// The open dialog; a new one replaces it and `ClearDialog` closes it. A registry dialog configuration sent
    /// shows inline; only the built-in ones stay `Shown::Registry`.
    pub dialog: Option<Shown>,
    dialogs: Vec<Option<Dialog>>,
    /// Unlocked recipes as the book shows them, in the order they came.
    pub recipe_book: Vec<BookRecipe>,
    /// The message the last packet carried (chat, system text, our death), until the next packet.
    pub message: Option<Message>,
    chat_types: Vec<String>,
    pub(crate) chat_state: Mutex<ChatState>,
    loading: Loading,
    /// The server sent the client back to configuration: what it has sent so far.
    reconfiguring: Option<Registries>,
}

impl Client {
    /// Joins an offline-mode server as `name`.
    pub async fn join_offline(host: &str, port: u16, name: &str) -> Result<Self, Error> {
        Self::join(host, port, name, offline_uuid(name), None, Mutex::default()).await
    }

    /// Joins any server as the account's Java profile.
    pub async fn join_online(host: &str, port: u16, account: &Account) -> Result<Self, Error> {
        let player = account.java_profile().await?;
        let uuid = u128::from_str_radix(&player.uuid, 16).map_err(|e| Error::Crypto(format!("profile id {}: {e}", player.uuid)))?;
        let chat_state = ChatState::fetch(account, &player).await;
        Self::join(host, port, &player.name.clone(), uuid, Some((account, &player)), chat_state).await
    }

    async fn join(host: &str, port: u16, name: &str, uuid: u128, session: Session<'_>, chat_state: Mutex<ChatState>) -> Result<Self, Error> {
        let mut c = Connection::new(TcpStream::connect((host, port)).await?);
        let intention = Intention { protocol: VarInt(PROTOCOL_VERSION), host: host.to_owned(), port, intent: VarInt(INTENT_LOGIN) };
        c.send(ids::handshake::serverbound::INTENTION, &intention).await?;
        c.send(ids::login::serverbound::HELLO, &LoginHello { name: name.to_owned(), uuid }).await?;
        let profile = login(&mut c, session).await?;
        let mut cookies = Cookies::new();
        let registries = configure(&mut c, &mut cookies).await?;
        let mut client = Self {
            link: Link::spawn(c, cookies),
            uuid: profile.uuid,
            name: profile.name,
            pose: Pose::default(),
            world: World::default(),
            movement: Movement::default(),
            actions: Actions::default(),
            inventory: Inventory::default(),
            stonecutter: Vec::new(),
            dialog: None,
            dialogs: Vec::new(),
            recipe_book: Vec::new(),
            message: None,
            chat_types: Vec::new(),
            chat_state,
            loading: Loading::default(),
            reconfiguring: None,
        };
        client.install(registries);
        Ok(client)
    }

    fn install(&mut self, registries: Registries) {
        self.inventory.enchantments = registries.enchantments.into();
        self.dialogs = registries.dialogs;
        self.chat_types = registries.chat_types;
    }

    /// The next play packet, already applied to the world and movement. Cancel-safe.
    pub async fn next(&mut self) -> Result<Packet, Error> {
        loop {
            let received = self.link.recv().await.ok_or(Error::Closed)??;
            if let Some(packet) = self.receive(received)? {
                return Ok(packet);
            }
        }
    }

    /// Like [`Client::next`] for a packet that has already arrived.
    pub fn try_next(&mut self) -> Option<Result<Packet, Error>> {
        loop {
            match self.link.try_recv()?.and_then(|received| self.receive(received)) {
                Ok(Some(packet)) => return Some(Ok(packet)),
                Ok(None) => {}
                Err(e) => return Some(Err(e)),
            }
        }
    }

    /// Applies a play packet, or takes a reconfiguration's packet (`None`: nothing for the caller).
    fn receive(&mut self, received: Received) -> Result<Option<Packet>, Error> {
        match received {
            Received::Play(packet) => self.apply(&packet).map(|()| Some(packet)),
            Received::Configuration(id, body) => {
                self.reconfigure(id, &body)?;
                Ok(None)
            }
        }
    }

    /// The server's configuration packets after `StartConfiguration` (the link answers them).
    fn reconfigure(&mut self, id: i32, body: &[u8]) -> Result<(), Error> {
        use ids::configuration::clientbound as from;
        match id {
            from::DISCONNECT => return Err(Error::Disconnected(format!("{body:02x?}"))),
            from::FINISH_CONFIGURATION => {
                let registries = self.reconfiguring.take().unwrap_or_default();
                self.install(registries);
            }
            _ => self.reconfiguring.get_or_insert_default().collect(id, body)?,
        }
        Ok(())
    }

    /// The server took the client back to configuration; play resumes with a new `Login`.
    pub fn configuring(&self) -> bool {
        self.reconfiguring.is_some()
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
                let to = self.movement.teleported(&position);
                self.pose = Pose { position: to.pos, y_rot: to.y_rot, x_rot: to.x_rot };
                let Pose { position: [x, y, z], y_rot, x_rot } = self.pose;
                self.send(to::ACCEPT_TELEPORTATION, &AcceptTeleportation { id: position.id, x, y, z, y_rot, x_rot })?;
            }
            from::GAME_EVENT if body.first() == Some(&LEVEL_CHUNKS_LOAD_START) => self.loading.chunks_started(),
            from::UPDATE_RECIPES => self.stonecutter = read_update_recipes(&mut &body[..])?,
            from::SHOW_DIALOG => {
                let shown = read_show_dialog(&mut &body[..])?;
                self.dialog = Some(match shown {
                    Shown::Registry(n) => self.dialogs.get(n).cloned().flatten().map_or(shown, Shown::Inline),
                    inline => inline,
                });
            }
            from::CLEAR_DIALOG => self.dialog = None,
            from::RECIPE_BOOK_ADD | from::RECIPE_BOOK_REMOVE => self.update_book(id, body)?,
            from::DISCONNECT => return Err(Error::Disconnected(format!("{body:02x?}"))),
            // `handleLogin` makes a new player: after a reconfiguration this is the only reset.
            from::LOGIN => {
                self.loading = Loading::default();
                self.movement = Movement::default();
            }
            // `setClientLoaded(false)` and a new `LevelLoadTracker`.
            from::RESPAWN => self.loading = Loading::default(),
            // `clearClientLevel`: no level or player until the next `Login`.
            from::START_CONFIGURATION => {
                self.reconfiguring = Some(Registries::default());
                self.loading = Loading::default();
            }
            _ => {}
        }
        // A message we can't read is dropped rather than ending the connection.
        self.message = messages::read(id, body, &self.chat_types, self.world.player_id).ok().flatten();
        self.apply_chat(id, body)?;
        self.world.apply(id, body)?;
        self.apply_riding(id, body)?;
        self.movement.apply(id, body, self.world.player_id)?;
        self.movement.apply_status(id, body, self.world.player_id)?;
        self.actions.apply(id, body)?;
        self.inventory.apply(id, body)?;
        Ok(())
    }

    /// The player is in the level: `PlayerLoaded` went out, and it ticks and moves (see `loading.rs`).
    pub fn loaded(&self) -> bool {
        self.loading == Loading::Loaded
    }

    /// Advances the level load by a tick, sending `PlayerLoaded` when it completes.
    pub(crate) fn tick_loading(&mut self) -> Result<(), Error> {
        let at = self.movement.player.pos.map(|v| v.floor() as i32);
        let ready = acacia_java_physics::Blocks::has_chunk(&self.world, at) || self.movement.dead();
        if self.loading.tick(ready) {
            self.send(ids::play::serverbound::PLAYER_LOADED, &Empty {})?;
        }
        Ok(())
    }
}

/// `ClientboundGameEventPacket.LEVEL_CHUNKS_LOAD_START`.
const LEVEL_CHUNKS_LOAD_START: u8 = 13;

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
}
