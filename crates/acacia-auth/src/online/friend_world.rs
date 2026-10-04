//! A friend's world as its MPSD activity handle and session describe it (docs/research/friends-join.md §3).

use serde_json::{Value, json};

/// Minecraft's Xbox service config id and lobby session template.
pub const MINECRAFT_SCID: &str = "4fc10100-5f7a-4470-899b-280835760c07";
pub const MINECRAFT_TEMPLATE: &str = "MinecraftLobby";

/// Names one MPSD session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionRef {
    pub scid: String,
    pub template_name: String,
    pub name: String,
}

impl SessionRef {
    pub fn url(&self) -> String {
        format!(
            "https://sessiondirectory.xboxlive.com/serviceconfigs/{}/sessionTemplates/{}/sessions/{}",
            self.scid, self.template_name, self.name
        )
    }

    pub(crate) fn to_json(&self) -> Value {
        json!({"scid": self.scid, "templateName": self.template_name, "name": self.name})
    }

    fn from_json(v: &Value) -> Option<Self> {
        let s = |k: &str| v.get(k).and_then(Value::as_str).map(str::to_owned);
        Some(Self { scid: s("scid")?, template_name: s("templateName")?, name: s("name")? })
    }
}

/// `SupportedConnections[].ConnectionType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionKind {
    /// 7: JSON-RPC signaling to the host's `PmsgId`.
    SignalingJsonRpc,
    /// 3: legacy WebSocket signaling to the host's `NetherNetId`.
    SignalingLegacy,
    /// 4: LAN discovery only.
    Lan,
    Other(i64),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorldConnection {
    pub kind: ConnectionKind,
    /// The host's NetherNet id (a u64, sent as a JSON number or string).
    pub nethernet_id: Option<String>,
    /// The host's player messaging id (UUID) for JSON-RPC signaling.
    pub pmsg_id: Option<String>,
    pub host_ip: Option<String>,
    pub host_port: u16,
}

impl WorldConnection {
    fn from_json(v: &Value) -> Self {
        let kind = match v.get("ConnectionType").and_then(Value::as_i64).unwrap_or(-1) {
            7 => ConnectionKind::SignalingJsonRpc,
            3 => ConnectionKind::SignalingLegacy,
            4 => ConnectionKind::Lan,
            n => ConnectionKind::Other(n),
        };
        Self {
            kind,
            nethernet_id: id_string(v.get("NetherNetId")),
            pmsg_id: id_string(v.get("PmsgId")),
            host_ip: id_string(v.get("HostIpAddress")),
            host_port: v.get("HostPort").and_then(Value::as_u64).and_then(|p| u16::try_from(p).ok()).unwrap_or(0),
        }
    }
}

/// A world a followed player is hosting, from `handles/query`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FriendWorld {
    /// The activity handle id that joins go through.
    pub handle_id: String,
    pub session: SessionRef,
    pub owner_xuid: String,
    pub host_name: String,
    pub world_name: String,
    pub version: String,
    /// The host's network protocol; vanilla refuses a mismatch before dialing.
    pub protocol: i32,
    pub members: u32,
    pub max_members: u32,
    /// `relatedInfo.closed`: the session takes no new members.
    pub closed: bool,
    pub connections: Vec<WorldConnection>,
}

impl FriendWorld {
    /// None for handles of other templates or without the fields a join needs.
    pub(crate) fn from_handle(v: &Value) -> Option<Self> {
        let session = SessionRef::from_json(v.get("sessionRef")?)?;
        let custom = v.get("customProperties").filter(|c| c.is_object())?;
        let s = |k: &str| custom.get(k).and_then(Value::as_str).unwrap_or_default().to_owned();
        let n = |k: &str| custom.get(k).and_then(Value::as_u64).and_then(|n| u32::try_from(n).ok()).unwrap_or(0);
        let owner = v.get("ownerXuid").and_then(Value::as_str).map(str::to_owned).unwrap_or_else(|| s("ownerId"));
        Some(Self {
            handle_id: v.get("id")?.as_str()?.to_owned(),
            session,
            owner_xuid: owner,
            host_name: s("hostName"),
            world_name: s("worldName"),
            version: s("version"),
            protocol: custom.get("protocol").and_then(Value::as_i64).and_then(|p| i32::try_from(p).ok()).unwrap_or(0),
            members: n("MemberCount"),
            max_members: n("MaxMemberCount"),
            closed: v.pointer("/relatedInfo/closed").and_then(Value::as_bool).unwrap_or(false),
            connections: connections(custom),
        })
    }
}

/// `customProperties.SupportedConnections`, from a handle's or a session document's custom properties.
pub fn connections(custom: &Value) -> Vec<WorldConnection> {
    custom
        .get("SupportedConnections")
        .and_then(Value::as_array)
        .map(|a| a.iter().map(WorldConnection::from_json).collect())
        .unwrap_or_default()
}

/// The nonce the host published for `xuid` in a session document, once it has.
pub fn session_nonce(session: &Value, xuid: &str) -> Option<String> {
    let nonce = session.pointer("/properties/custom/nonces")?.get(xuid)?.as_str()?;
    (!nonce.is_empty()).then(|| nonce.to_owned())
}

pub(crate) fn parse_activities(v: &Value) -> Vec<FriendWorld> {
    let results = v.get("results").and_then(Value::as_array).map(Vec::as_slice).unwrap_or_default();
    results
        .iter()
        .filter(|h| h.pointer("/sessionRef/templateName").and_then(Value::as_str) == Some(MINECRAFT_TEMPLATE))
        .filter_map(FriendWorld::from_handle)
        .collect()
}

fn id_string(v: Option<&Value>) -> Option<String> {
    match v? {
        Value::String(s) if !s.is_empty() => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}
