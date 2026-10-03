//! Realms REST: list, invite codes and join targets (gophertunnel `minecraft/realms`,
//! prismarine-realms). Spec: acacia-client docs/research/nethernet-signaling.md §2a.

use std::time::Duration;

use rand_core::RngCore;
use reqwest::RequestBuilder;
use serde::Deserialize;
use serde_json::Value;

use super::client::AuthClient;
use super::xbox::XboxToken;
use crate::{Error, Result};

/// Discovery's `realms_frontend_bedrock_legacy`, what vanilla calls (capture 2026-10-02); the XSTS
/// relying party is still the pocket one.
const BASE: &str = "https://bedrock.frontendlegacy.realms.minecraft-services.net";
pub const REALMS_RELYING_PARTY: &str = "https://pocket.realms.minecraft.net/";
/// Keep equal to acacia-proto's protocol for `GAME_VERSION`.
const NETWORK_PROTOCOL: &str = "2193";
/// TODO: per-build id seen from Windows 1.26.52; capture the Android build's value.
const CLIENT_REF: &str = "54116bb5ee0b7aee0b5591efbcc8bc3cf64f168a";
/// TODO: vanilla Windows sends `Windows`; bots log in as Android, capture what Android sends.
const CLIENT_PLATFORM: &str = "Android";
/// A sleeping realm answers join with 503 and `retry-after: 3`; vanilla waits that plus 100-160 ms.
const JOIN_ATTEMPTS: u32 = 20;
const JOIN_RETRY: Duration = Duration::from_secs(3);

/// One `pingRegions` entry of the join body: measured latency to a region's QoS beacon.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct PingRegion {
    #[serde(rename = "latencyMs")]
    pub latency_ms: u32,
    pub region: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Realm {
    pub id: i64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub owner: Option<String>,
    /// `OPEN`, `CLOSED`, …
    #[serde(default)]
    pub state: String,
    #[serde(default)]
    pub expired: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RealmProtocol {
    /// `DEFAULT`: `address` is `host:port` over RakNet.
    RakNet,
    /// `NETHERNET`: `address` is the host's NetherNet id; legacy WebSocket signaling.
    NetherNet,
    /// `NETHERNET_JSONRPC`: `address` is the peer id for JSON-RPC signaling.
    NetherNetJsonRpc,
    Other(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RealmJoin {
    pub address: String,
    pub protocol: RealmProtocol,
    /// `sessionRegionData.regionName`. Not a signaling host: `signal-<region>` names don't resolve
    /// (checked 2026-10-02); signaling goes to discovery's `signaling.prod`.
    pub region: Option<String>,
}

impl RealmJoin {
    fn from_json(v: &Value) -> Self {
        let s = |p: &str| v.pointer(p).and_then(Value::as_str).map(str::to_owned);
        let protocol = match s("/networkProtocol").as_deref() {
            None | Some("DEFAULT") => RealmProtocol::RakNet,
            Some("NETHERNET") => RealmProtocol::NetherNet,
            Some("NETHERNET_JSONRPC") => RealmProtocol::NetherNetJsonRpc,
            Some(other) => RealmProtocol::Other(other.to_owned()),
        };
        Self { address: s("/address").unwrap_or_default(), protocol, region: s("/sessionRegionData/regionName") }
    }
}

/// An invite code or `https://realms.gg/<code>` link.
fn invite_code(code: &str) -> &str {
    code.trim().trim_end_matches('/').rsplit('/').next().unwrap_or(code)
}

/// Vanilla's header set in vanilla's order (content-type even on GETs, no accept).
fn request(client: &AuthClient, xsts: &XboxToken, method: reqwest::Method, path: &str) -> RequestBuilder {
    client
        .http()
        .client
        .request(method, format!("{BASE}{path}"))
        .header("content-type", "application/json")
        .header("authorization", xsts.authorization())
        .header("user-agent", "libhttpclient/1.0.0.0")
        .header("charset", "utf-8")
        .header("client-ref", CLIENT_REF)
        .header("client-version", &client.config().game_version)
        .header("x-clientplatform", CLIENT_PLATFORM)
        .header("x-networkprotocolversion", NETWORK_PROTOCOL)
}

pub(crate) async fn list(client: &AuthClient, xsts: &XboxToken) -> Result<Vec<Realm>> {
    #[derive(Deserialize)]
    struct Worlds {
        #[serde(default)]
        servers: Vec<Realm>,
    }
    let url = format!("{BASE}/worlds");
    let worlds: Worlds = client.http().json(request(client, xsts, reqwest::Method::GET, "/worlds"), &url).await?;
    Ok(worlds.servers)
}

/// Looks up the realm behind an invite code without joining it.
pub(crate) async fn by_invite(client: &AuthClient, xsts: &XboxToken, code: &str) -> Result<Realm> {
    let path = format!("/worlds/v1/link/{}", invite_code(code));
    client.http().json(request(client, xsts, reqwest::Method::GET, &path), &format!("{BASE}{path}")).await
}

/// Accepts an invite code, making the account a member of the realm.
pub(crate) async fn accept_invite(client: &AuthClient, xsts: &XboxToken, code: &str) -> Result<Realm> {
    let path = format!("/invites/v1/link/accept/{}", invite_code(code));
    client.http().json(request(client, xsts, reqwest::Method::POST, &path), &format!("{BASE}{path}")).await
}

/// The join target, waiting while the realm starts up (503). `ping_regions` are QoS latencies in
/// discovery's `qos-beacons` order; vanilla sends all of them, the same body on every retry.
pub(crate) async fn join(
    client: &AuthClient,
    xsts: &XboxToken,
    realm_id: i64,
    ping_regions: &[PingRegion],
) -> Result<RealmJoin> {
    let path = format!("/worlds/{realm_id}/join");
    let url = format!("{BASE}{path}");
    let body = serde_json::json!({"joinIntention": "VANILLA", "pingRegions": ping_regions}).to_string();
    for attempt in 1..=JOIN_ATTEMPTS {
        let req = request(client, xsts, reqwest::Method::POST, &path).body(body.clone());
        match client.http().json::<Value>(req, &url).await {
            Ok(v) => return Ok(RealmJoin::from_json(&v)),
            Err(Error::Status { status: 503, .. }) if attempt < JOIN_ATTEMPTS => {
                tracing::debug!(realm_id, attempt, "realm starting, retrying join");
                let jitter = Duration::from_millis(100 + u64::from(rand_core::OsRng.next_u32() % 61));
                tokio::time::sleep(JOIN_RETRY + jitter).await;
            }
            Err(e) => return Err(e),
        }
    }
    unreachable!("the last attempt returns")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_join_targets() {
        let j = RealmJoin::from_json(&json!({
            "address": "abc-123", "networkProtocol": "NETHERNET_JSONRPC", "pendingUpdate": false,
            "sessionRegionData": {"regionName": "WestEurope", "serviceQuality": 1}
        }));
        assert_eq!((j.protocol, j.region.as_deref()), (RealmProtocol::NetherNetJsonRpc, Some("WestEurope")));
        let r = RealmJoin::from_json(&json!({"address": "1.2.3.4:19132", "networkProtocol": "DEFAULT"}));
        assert_eq!((r.protocol, r.address.as_str(), r.region), (RealmProtocol::RakNet, "1.2.3.4:19132", None));
    }

    #[test]
    fn strips_invite_links() {
        assert_eq!(invite_code("https://realms.gg/AbCdEf"), "AbCdEf");
        assert_eq!(invite_code(" AbCdEf "), "AbCdEf");
    }
}
