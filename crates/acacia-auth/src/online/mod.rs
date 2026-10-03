//! Network auth (feature `online`).

mod account;
mod cache;
mod client;
mod config;
mod friend_world;
#[cfg(test)]
mod friend_world_tests;
mod http;
mod jwks;
mod live;
#[cfg(test)]
mod live_tests;
mod minecraft;
mod mpsd;
mod nsal;
mod realms;
pub mod sign;
mod tokens;
mod xbox;
mod xsapi;

pub use account::Account;
pub use cache::{CacheError, FileTokenCache, MemoryTokenCache, Stored, TokenCache, Versioned};
pub use tokens::{CachedCredentials, CachedTokens};
pub use client::AuthClient;
pub use config::{AuthConfig, GAME_VERSION, Title};
pub use live::{DeviceCodePrompt, MsaToken};
pub use minecraft::{PlayFabSession, QosBeacon, ServiceToken, SignalingEnvironment};
pub use friend_world::{
    ConnectionKind, FriendWorld, MINECRAFT_SCID, MINECRAFT_TEMPLATE, SessionRef, WorldConnection, connections,
    session_nonce,
};
pub use mpsd::RTA_CONNECTIONS_URI;
pub use realms::{PingRegion, Realm, RealmJoin, RealmProtocol, REALMS_RELYING_PARTY};
pub use xbox::{SisuTokens, XboxToken};
