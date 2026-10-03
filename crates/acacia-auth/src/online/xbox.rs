//! Xbox Live device token, SISU authorize and XSTS (go-xsapi `xasd`, `sisu`, `xsts`).

use p256::ecdsa::SigningKey;
use serde::{Deserialize, Serialize};
use serde_json::json;

use super::config::{Title, endpoints as ep};
use super::http::Http;
use super::sign::proof_key_jwk;
use crate::jwt::now_unix;
use crate::{Error, Result};

/// A device, title, user or XSTS token.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct XboxToken {
    pub token: String,
    /// Unix seconds (`NotAfter`).
    pub not_after: i64,
    /// User hash (`xui[0].uhs`) for user/XSTS tokens.
    pub user_hash: Option<String>,
    pub xuid: Option<String>,
    pub gamertag: Option<String>,
}

impl XboxToken {
    /// go-xsapi treats tokens as expired one minute early.
    pub fn is_valid(&self) -> bool {
        self.not_after - 60 > now_unix()
    }

    /// `XBL3.0 x=<uhs>;<token>` for the `Authorization` header.
    pub fn authorization(&self) -> String {
        format!("XBL3.0 x={};{}", self.user_hash.as_deref().unwrap_or(""), self.token)
    }
}

/// SISU authorize result: title, user and `http://xboxlive.com` XSTS tokens at once.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SisuTokens {
    pub title: XboxToken,
    pub user: XboxToken,
    pub authorization: XboxToken,
}

impl SisuTokens {
    pub fn is_valid(&self) -> bool {
        self.title.is_valid() && self.user.is_valid() && self.authorization.is_valid()
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct WireToken {
    not_after: String,
    token: String,
    #[serde(default)]
    display_claims: WireClaims,
}

#[derive(Deserialize, Default)]
struct WireClaims {
    #[serde(default)]
    xui: Vec<WireUser>,
}

#[derive(Deserialize)]
struct WireUser {
    uhs: Option<String>,
    xid: Option<String>,
    gtg: Option<String>,
}

impl WireToken {
    fn into_token(self, endpoint: &str) -> Result<XboxToken> {
        let not_after = time::OffsetDateTime::parse(
            &self.not_after,
            &time::format_description::well_known::Rfc3339,
        )
        .map_err(|e| Error::Protocol {
            endpoint: endpoint.into(),
            reason: format!("NotAfter {:?}: {e}", self.not_after),
        })?
        .unix_timestamp();
        let user = self.display_claims.xui.into_iter().next();
        Ok(XboxToken {
            token: self.token,
            not_after,
            user_hash: user.as_ref().and_then(|u| u.uhs.clone()),
            xuid: user.as_ref().and_then(|u| u.xid.clone()),
            gamertag: user.and_then(|u| u.gtg),
        })
    }
}

/// `POST device.auth.xboxlive.com/device/authenticate` (ProofOfPossession).
pub(crate) async fn device_token(http: &Http, title: Title, key: &SigningKey) -> Result<XboxToken> {
    let (device_type, version) = title.device();
    // Android and Nintendo use a braced lowercase UUID and no SerialNumber (go-xsapi `deviceID`).
    let id = format!("{{{}}}", uuid::Uuid::new_v4());
    let body = json!({
        "RelyingParty": "http://auth.xboxlive.com",
        "TokenType": "JWT",
        "Properties": {
            "AuthMethod": "ProofOfPossession",
            "Id": id,
            "DeviceType": device_type,
            "Version": version,
            "ProofKey": proof_key_jwk(key),
        },
    });
    let req = http.xbox_post(
        ep::DEVICE_AUTH,
        title.user_agent(),
        Some("1"),
        None,
        serde_json::to_vec(&body)?,
        key,
    );
    let t: WireToken = http.json(req, ep::DEVICE_AUTH).await?;
    t.into_token(ep::DEVICE_AUTH)
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct SisuResponse {
    title_token: WireToken,
    user_token: WireToken,
    authorization_token: WireToken,
}

/// `POST sisu.xboxlive.com/authorize`: MSA access token + device token → title, user and
/// `http://xboxlive.com` XSTS tokens.
pub(crate) async fn sisu_authorize(
    http: &Http,
    title: Title,
    key: &SigningKey,
    msa_access_token: &str,
    device: &XboxToken,
) -> Result<SisuTokens> {
    let body = json!({
        "AccessToken": format!("t={msa_access_token}"),
        "AppId": title.client_id(),
        "DeviceToken": device.token,
        "ProofKey": proof_key_jwk(key),
        "RelyingParty": ep::RP_XBOXLIVE,
        "Sandbox": "RETAIL",
        "SiteName": "user.auth.xboxlive.com",
        "UseModernGamertag": true,
    });
    let req =
        http.xbox_post(ep::SISU_AUTHORIZE, title.user_agent(), None, None, serde_json::to_vec(&body)?, key);
    let r: SisuResponse = http.json(req, ep::SISU_AUTHORIZE).await?;
    Ok(SisuTokens {
        title: r.title_token.into_token(ep::SISU_AUTHORIZE)?,
        user: r.user_token.into_token(ep::SISU_AUTHORIZE)?,
        authorization: r.authorization_token.into_token(ep::SISU_AUTHORIZE)?,
    })
}

/// `POST xsts.auth.xboxlive.com/xsts/authorize` for one relying party.
pub(crate) async fn xsts(
    http: &Http,
    title: Title,
    key: &SigningKey,
    relying_party: &str,
    device: &XboxToken,
    sisu: &SisuTokens,
) -> Result<XboxToken> {
    let body = json!({
        "RelyingParty": relying_party,
        "TokenType": "JWT",
        "Properties": {
            "SandboxId": "RETAIL",
            "DeviceToken": device.token,
            "TitleToken": sisu.title.token,
            "UserTokens": [sisu.user.token],
        },
    });
    let req = http.xbox_post(
        ep::XSTS_AUTHORIZE,
        title.user_agent(),
        Some("1"),
        None,
        serde_json::to_vec(&body)?,
        key,
    );
    let t: WireToken = http.json(req, ep::XSTS_AUTHORIZE).await?;
    let token = t.into_token(ep::XSTS_AUTHORIZE)?;
    if token.user_hash.is_none() {
        return Err(Error::Protocol {
            endpoint: ep::XSTS_AUTHORIZE.into(),
            reason: "XSTS token has no user claims".into(),
        });
    }
    Ok(token)
}
