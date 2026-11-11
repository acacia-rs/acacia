//! The configuration phase, at join and when the server reconfigures (`StartConfiguration`, as a proxy switching
//! backends does), and the answers it shares with play: cookies and resource packs.

use std::collections::HashMap;

use acacia_java_proto::dialog::Dialog;
use acacia_java_proto::packets::{
    Brand, ClientInformation, CookieRequest, CookieResponse, Empty, KeepAlive, Ping, ResourcePackPush, ResourcePackResponse, SelectKnownPacks,
    StoreCookie,
};
use acacia_java_proto::tag::Tag;
use acacia_java_proto::{VarInt, Wire, ids, read_varint};

use crate::frame::{Error, encode};

use ids::configuration::{clientbound as from, serverbound as to};

/// Cookies the server stored (`StoreCookie`), by key; sent back on `CookieRequest`.
pub(crate) type Cookies = HashMap<String, Vec<u8>>;

/// What configuration leaves the client: enchantment names, and dialogs whose data the server sent (datapack ones;
/// the built-in packs both sides claim come without), by network id.
#[derive(Default)]
pub(crate) struct Registries {
    pub enchantments: Vec<String>,
    pub dialogs: Vec<Option<Dialog>>,
    pub chat_types: Vec<String>,
}

impl Registries {
    /// Takes what a configuration packet carries for the client.
    pub(crate) fn collect(&mut self, id: i32, mut input: &[u8]) -> Result<(), Error> {
        if id != from::REGISTRY_DATA {
            return Ok(());
        }
        let input = &mut input;
        match String::read(input)?.as_str() {
            ENCHANTMENT_REGISTRY => self.enchantments = names(registry_entries(input)?),
            CHAT_TYPE_REGISTRY => self.chat_types = names(registry_entries(input)?),
            DIALOG_REGISTRY => self.dialogs = registry_entries(input)?.iter().map(|(_, data)| data.as_ref().and_then(Dialog::from_tag)).collect(),
            _ => {}
        }
        Ok(())
    }
}

const ENCHANTMENT_REGISTRY: &str = "minecraft:enchantment";
const DIALOG_REGISTRY: &str = "minecraft:dialog";
const CHAT_TYPE_REGISTRY: &str = "minecraft:chat_type";

fn names(entries: Vec<(String, Option<Tag>)>) -> Vec<String> {
    entries.into_iter().map(|(name, _)| name).collect()
}

/// `RegistryData` entries after the registry key: ids in network order, each with optional data (absent for packs
/// both sides know).
fn registry_entries(input: &mut &[u8]) -> Result<Vec<(String, Option<Tag>)>, Error> {
    let mut entries = Vec::new();
    for _ in 0..read_varint(input)? {
        let name = String::read(input)?;
        let data = if bool::read(input)? { Some(Tag::read_root(input)?.ok_or(acacia_java_proto::Error::Invalid("registry entry NBT"))?) } else { None };
        entries.push((name, data));
    }
    Ok(entries)
}

/// What a vanilla client sends on entering configuration from login: its brand, then its options.
pub(crate) fn hello() -> [Vec<u8>; 2] {
    let brand = Brand { channel: "minecraft:brand".into(), brand: "vanilla".into() };
    let information = ClientInformation {
        language: "en_us".into(),
        view_distance: 12,
        chat_visibility: VarInt(0),
        chat_colors: true,
        model_customisation: 0x7f,
        main_hand: VarInt(1),
        text_filtering: false,
        allows_listing: true,
        particles: VarInt(0),
    };
    [encode(to::CUSTOM_PAYLOAD, &brand), encode(to::CLIENT_INFORMATION, &information)]
}

/// The client's answers to a configuration packet, encoded, in order. `FinishConfiguration`'s acknowledgement is
/// among them: after it, the connection is back in play.
pub(crate) fn answer(id: i32, body: &[u8], cookies: &mut Cookies) -> Result<Vec<Vec<u8>>, Error> {
    let input = &mut &body[..];
    Ok(match id {
        from::KEEP_ALIVE => vec![encode(to::KEEP_ALIVE, &KeepAlive::read(input)?)],
        from::PING => vec![encode(to::PONG, &Ping::read(input)?)],
        // Claiming the server's own packs means it omits the vanilla registry contents.
        from::SELECT_KNOWN_PACKS => vec![encode(to::SELECT_KNOWN_PACKS, &SelectKnownPacks::read(input)?)],
        from::CODE_OF_CONDUCT => vec![encode(to::ACCEPT_CODE_OF_CONDUCT, &Empty {})],
        from::FINISH_CONFIGURATION => vec![encode(to::FINISH_CONFIGURATION, &Empty {})],
        _ => answer_common(&CONFIGURATION, id, body, cookies)?,
    })
}

/// The ids of the packets configuration and play both answer.
pub(crate) struct CommonIds {
    cookie_request: i32,
    store_cookie: i32,
    resource_pack_push: i32,
    cookie_response: i32,
    resource_pack: i32,
}

pub(crate) const CONFIGURATION: CommonIds = CommonIds {
    cookie_request: from::COOKIE_REQUEST,
    store_cookie: from::STORE_COOKIE,
    resource_pack_push: from::RESOURCE_PACK_PUSH,
    cookie_response: to::COOKIE_RESPONSE,
    resource_pack: to::RESOURCE_PACK,
};

pub(crate) const PLAY: CommonIds = CommonIds {
    cookie_request: ids::play::clientbound::COOKIE_REQUEST,
    store_cookie: ids::play::clientbound::STORE_COOKIE,
    resource_pack_push: ids::play::clientbound::RESOURCE_PACK_PUSH,
    cookie_response: ids::play::serverbound::COOKIE_RESPONSE,
    resource_pack: ids::play::serverbound::RESOURCE_PACK,
};

/// `ServerboundResourcePackPacket.Action`: what a client reports for a pack it took.
const PACK_LOADED: [i32; 3] = [ACCEPTED, DOWNLOADED, SUCCESSFULLY_LOADED];
const SUCCESSFULLY_LOADED: i32 = 0;
const ACCEPTED: i32 = 3;
const DOWNLOADED: i32 = 4;

/// Answers cookies from the store and takes every resource pack (reporting it loaded without fetching it, since a
/// required pack declined is a kick); stores `StoreCookie`s.
pub(crate) fn answer_common(ids: &CommonIds, id: i32, mut body: &[u8], cookies: &mut Cookies) -> Result<Vec<Vec<u8>>, Error> {
    let input = &mut body;
    Ok(if id == ids.cookie_request {
        let key = CookieRequest::read(input)?.key;
        let payload = cookies.get(&key).cloned();
        vec![encode(ids.cookie_response, &CookieResponse { key, payload })]
    } else if id == ids.store_cookie {
        let cookie = StoreCookie::read(input)?;
        cookies.insert(cookie.key, cookie.payload);
        Vec::new()
    } else if id == ids.resource_pack_push {
        let pack = ResourcePackPush::read(input)?.id;
        PACK_LOADED.iter().map(|&action| encode(ids.resource_pack, &ResourcePackResponse { id: pack, action: VarInt(action) })).collect()
    } else {
        Vec::new()
    })
}

#[cfg(test)]
#[path = "configuration_tests.rs"]
mod configuration_tests;
