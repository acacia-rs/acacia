//! Microsoft account (login.live.com) device code flow and refresh, as golang.org/x/oauth2 does
//! it for go-xsapi `sisu.Config` (form params, `AuthStyleInParams`).

use std::time::Duration;

use serde::{Deserialize, Serialize};

use super::config::{Title, endpoints as ep};
use super::http::Http;
use crate::jwt::now_unix;
use crate::{Error, Result};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DeviceCodePrompt {
    pub user_code: String,
    pub verification_uri: String,
    /// Seconds the code stays valid from `issued_at`.
    pub expires_in: u64,
    /// Minimum seconds between polls.
    pub interval: u64,
    pub device_code: String,
    pub issued_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MsaToken {
    pub access_token: String,
    pub refresh_token: Option<String>,
    /// Unix seconds.
    pub expires_at: i64,
    pub user_id: Option<String>,
}

impl MsaToken {
    pub fn is_valid(&self) -> bool {
        self.expires_at - 60 > now_unix()
    }
}

#[derive(Deserialize)]
struct DeviceCodeResponse {
    user_code: String,
    device_code: String,
    verification_uri: String,
    expires_in: u64,
    #[serde(default)]
    interval: Option<u64>,
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: Option<String>,
    expires_in: i64,
    user_id: Option<String>,
}

#[derive(Deserialize)]
struct OAuthError {
    error: String,
    #[serde(default)]
    error_description: String,
}

pub(crate) async fn start_device_code(http: &Http, title: Title) -> Result<DeviceCodePrompt> {
    let req = http.client.post(ep::LIVE_DEVICE_CODE).form(&[
        ("client_id", title.client_id()),
        ("scope", ep::LIVE_SCOPE),
        ("response_type", "device_code"),
    ]);
    let r: DeviceCodeResponse = http.json(req, ep::LIVE_DEVICE_CODE).await?;
    Ok(DeviceCodePrompt {
        user_code: r.user_code,
        verification_uri: r.verification_uri,
        expires_in: r.expires_in,
        interval: r.interval.unwrap_or(5).max(1),
        device_code: r.device_code,
        issued_at: now_unix(),
    })
}

pub(crate) async fn poll_device_code(
    http: &Http,
    title: Title,
    prompt: &DeviceCodePrompt,
) -> Result<MsaToken> {
    let mut interval = prompt.interval;
    let deadline = prompt.issued_at + prompt.expires_in as i64;
    loop {
        tokio::time::sleep(Duration::from_secs(interval)).await;
        if now_unix() > deadline {
            return Err(Error::DeviceCodeExpired);
        }
        let req = http.client.post(ep::LIVE_TOKEN).form(&[
            ("client_id", title.client_id()),
            ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
            ("device_code", prompt.device_code.as_str()),
            ("scope", ep::LIVE_SCOPE),
        ]);
        match token_request(req).await {
            Ok(t) => return Ok(t),
            Err(Error::OAuth { code, .. }) if code == "authorization_pending" => {}
            Err(Error::OAuth { code, .. }) if code == "slow_down" => interval += 5,
            Err(Error::OAuth { code, .. }) if code == "expired_token" => {
                return Err(Error::DeviceCodeExpired);
            }
            Err(Error::OAuth { code, .. })
                if code == "access_denied" || code == "authorization_declined" =>
            {
                return Err(Error::DeviceCodeDeclined);
            }
            Err(e) => return Err(e),
        }
    }
}

pub(crate) async fn refresh(http: &Http, title: Title, token: &MsaToken) -> Result<MsaToken> {
    let refresh = token.refresh_token.as_deref().ok_or_else(|| Error::OAuth {
        code: "no_refresh_token".into(),
        description: "MSA token has no refresh token; run the device code flow again".into(),
    })?;
    let req = http
        .client
        .post(ep::LIVE_TOKEN)
        .header("User-Agent", title.user_agent())
        .form(&[
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh),
            ("scope", ep::LIVE_SCOPE),
            ("client_id", title.client_id()),
        ]);
    let mut t = token_request(req).await?;
    // live.com may omit a new refresh token; keep the old one then.
    if t.refresh_token.is_none() {
        t.refresh_token = token.refresh_token.clone();
    }
    Ok(t)
}

async fn token_request(req: reqwest::RequestBuilder) -> Result<MsaToken> {
    let resp = req.send().await?;
    let status = resp.status();
    let body = resp.bytes().await?;
    if !status.is_success() {
        return Err(match serde_json::from_slice::<OAuthError>(&body) {
            Ok(e) => Error::OAuth { code: e.error, description: e.error_description },
            Err(_) => Error::Status {
                endpoint: ep::LIVE_TOKEN.into(),
                status: status.as_u16(),
                body: String::from_utf8_lossy(&body).into_owned(),
            },
        });
    }
    let t: TokenResponse = serde_json::from_slice(&body)?;
    Ok(MsaToken {
        access_token: t.access_token,
        refresh_token: t.refresh_token,
        expires_at: now_unix() + t.expires_in,
        user_id: t.user_id,
    })
}
