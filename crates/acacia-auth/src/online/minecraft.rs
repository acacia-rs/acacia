//! Minecraft-side token requests (gophertunnel `auth.RequestMinecraftChain`, `service.Discover`,
//! go-playfab `LoginWithXbox`, `service.AuthorizationEnvironment.Token/MultiplayerToken`).

use p256::ecdsa::SigningKey as XboxKey;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::config::endpoints as ep;
use super::http::Http;
use super::xbox::XboxToken;
use crate::jwt::now_unix;
use crate::{Error, Result};

/// `serviceEnvironments.auth.prod` from discovery.
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct AuthEnvironment {
    #[serde(rename = "serviceUri")]
    pub service_uri: String,
    #[serde(rename = "playfabTitleId", alias = "playFabTitleId")]
    pub playfab_title_id: String,
}

impl AuthEnvironment {
    pub fn playfab_host(&self) -> String {
        format!("{}.playfabapi.com", self.playfab_title_id.to_lowercase())
    }
}

/// `serviceEnvironments.signaling.prod` from discovery: the NetherNet signaling service.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct SignalingEnvironment {
    #[serde(rename = "serviceUri")]
    pub service_uri: String,
    #[serde(rename = "stunUri", default)]
    pub stun_uri: Option<String>,
    #[serde(rename = "turnUri", default)]
    pub turn_uri: Option<String>,
}

/// A PlayFab QoS beacon from discovery's `qos-beacons.prod`: UDP port 3075, echoes `0xFFFF…` as `0x0000…`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QosBeacon {
    /// Region name as the realms join body spells it (`ukSouth`).
    pub region: String,
    pub host: String,
}

/// What this crate uses from service discovery.
#[derive(Debug, Clone)]
pub(crate) struct Discovery {
    pub auth: AuthEnvironment,
    pub signaling: Option<SignalingEnvironment>,
    /// In discovery's key order (alphabetical), the order vanilla sends `pingRegions` in.
    pub qos_beacons: Vec<QosBeacon>,
}

pub(crate) async fn discover(http: &Http, game_version: &str) -> Result<Discovery> {
    let url = format!("{}/{game_version}", ep::DISCOVERY);
    let req = http
        .client
        .get(&url)
        .header("Content-Type", "application/json")
        .header("User-Agent", ep::SERVICES_USER_AGENT);
    let v: Value = http.json(req, &url).await?;
    let env = v.pointer("/result/serviceEnvironments/auth/prod").cloned().ok_or_else(|| {
        Error::Protocol { endpoint: url.clone(), reason: "no auth.prod environment".into() }
    })?;
    let signaling = v
        .pointer("/result/serviceEnvironments/signaling/prod")
        .cloned()
        .and_then(|s| serde_json::from_value(s).ok());
    let qos_beacons = v
        .pointer("/result/serviceEnvironments/qos-beacons/prod")
        .and_then(Value::as_object)
        .map(|m| {
            m.iter()
                .filter_map(|(region, host)| Some(QosBeacon { region: region.clone(), host: host.as_str()?.to_owned() }))
                .collect()
        })
        .unwrap_or_default();
    Ok(Discovery { auth: serde_json::from_value(env)?, signaling, qos_beacons })
}

/// `POST multiplayer.minecraft.net/authentication` → the 2-JWT Mojang chain bound to
/// `identity_public_key` (client key SPKI, base64). No `Signature` header, like the game.
pub(crate) async fn mojang_chain(
    http: &Http,
    xsts: &XboxToken,
    identity_public_key: &str,
    game_version: &str,
) -> Result<Vec<String>> {
    #[derive(Deserialize)]
    struct Chain {
        chain: Vec<String>,
    }
    let req = http
        .client
        .post(ep::MOJANG_CHAIN)
        .header("User-Agent", "MCPE/Android")
        .header("Client-Version", game_version)
        .header("Content-Type", "application/json")
        .header("Authorization", xsts.authorization())
        .body(json!({ "identityPublicKey": identity_public_key }).to_string());
    let c: Chain = http.json(req, ep::MOJANG_CHAIN).await?;
    Ok(c.chain)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PlayFabSession {
    pub session_ticket: String,
    pub playfab_id: String,
    /// Unix seconds; go-playfab re-logs in after 24h minus 15 minutes.
    pub expires_at: i64,
}

impl PlayFabSession {
    pub fn is_valid(&self) -> bool {
        self.expires_at > now_unix()
    }
}

/// `POST https://<title>.playfabapi.com/Client/LoginWithXbox`, authorized and signed like every
/// XSAPI-transport request in gophertunnel.
pub(crate) async fn playfab_login(
    http: &Http,
    env: &AuthEnvironment,
    xsts: &XboxToken,
    xbox_key: &XboxKey,
    user_agent: &str,
) -> Result<PlayFabSession> {
    #[derive(Deserialize)]
    struct Data {
        #[serde(rename = "SessionTicket")]
        session_ticket: String,
        #[serde(rename = "PlayFabId")]
        playfab_id: String,
    }
    #[derive(Deserialize)]
    struct Envelope {
        data: Data,
    }
    let url = format!("https://{}/Client/LoginWithXbox", env.playfab_host());
    let auth = xsts.authorization();
    let body = json!({
        "TitleId": env.playfab_title_id,
        "CreateAccount": true,
        "XboxToken": auth,
    });
    let req = http.xbox_post(&url, user_agent, None, Some(&auth), serde_json::to_vec(&body)?, xbox_key);
    let r: Envelope = http.json(req, &url).await?;
    Ok(PlayFabSession {
        session_ticket: r.data.session_ticket,
        playfab_id: r.data.playfab_id,
        expires_at: now_unix() + 24 * 3600 - 15 * 60,
    })
}

/// Minecraft services `MCToken` (the `authorizationHeader` from session/start).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ServiceToken {
    pub authorization_header: String,
    /// Unix seconds.
    pub valid_until: i64,
}

impl ServiceToken {
    pub fn is_valid(&self) -> bool {
        self.valid_until - 60 > now_unix()
    }
}

/// `POST {serviceUri}/api/v1.0/session/start` with gophertunnel's default device config.
pub(crate) async fn session_start(
    http: &Http,
    env: &AuthEnvironment,
    playfab: &PlayFabSession,
    device_id: &str,
    game_version: &str,
) -> Result<ServiceToken> {
    #[derive(Deserialize)]
    struct Token {
        #[serde(rename = "authorizationHeader")]
        authorization_header: String,
        #[serde(rename = "validUntil")]
        valid_until: String,
    }
    let url = format!("{}/api/v1.0/session/start", env.service_uri.trim_end_matches('/'));
    let body = json!({
        "device": {
            "applicationType": "MinecraftPE",
            "capabilities": null,
            "gameVersion": game_version,
            "id": device_id,
            "memory": (16u64 << 30).to_string(),
            "platform": "Windows10",
            "playFabTitleId": env.playfab_title_id,
            "storePlatform": "uwp.store",
            "type": "Windows10",
        },
        "user": {
            "language": "en",
            "languageCode": "en-US",
            "regionCode": "US",
            "token": playfab.session_ticket,
            "tokenType": "PlayFab",
        },
    });
    let req = services_post(http, &url, &body).header("User-Agent", ep::SERVICES_USER_AGENT);
    let t: Token = result_of(http.json(req, &url).await?, &url)?;
    Ok(ServiceToken {
        authorization_header: t.authorization_header,
        valid_until: parse_time(&t.valid_until, &url)?,
    })
}

/// `POST {serviceUri}/api/v1.0/multiplayer/session/start` → JWT with `cpk` = `public_key`.
pub(crate) async fn multiplayer_token(
    http: &Http,
    env: &AuthEnvironment,
    service: &ServiceToken,
    public_key: &str,
) -> Result<(String, i64)> {
    #[derive(Deserialize)]
    struct Token {
        #[serde(rename = "signedToken")]
        signed_token: String,
        #[serde(rename = "validUntil")]
        valid_until: String,
    }
    let url = format!("{}/api/v1.0/multiplayer/session/start", env.service_uri.trim_end_matches('/'));
    let req = services_post(http, &url, &json!({ "publicKey": public_key }))
        .header("Authorization", &service.authorization_header);
    let t: Token = result_of(http.json(req, &url).await?, &url)?;
    Ok((t.signed_token, parse_time(&t.valid_until, &url)?))
}

fn services_post(http: &Http, url: &str, body: &Value) -> reqwest::RequestBuilder {
    http.client
        .post(url)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .body(body.to_string())
}

/// Unwraps the `{"result": ...}` envelope of minecraft-services responses.
fn result_of<T: serde::de::DeserializeOwned>(v: Value, url: &str) -> Result<T> {
    let inner = v.get("result").cloned().ok_or_else(|| Error::Protocol {
        endpoint: url.into(),
        reason: "missing result".into(),
    })?;
    Ok(serde_json::from_value(inner)?)
}

fn parse_time(s: &str, url: &str) -> Result<i64> {
    time::OffsetDateTime::parse(s, &time::format_description::well_known::Rfc3339)
        .map(|t| t.unix_timestamp())
        .map_err(|e| Error::Protocol { endpoint: url.into(), reason: format!("time {s:?}: {e}") })
}
