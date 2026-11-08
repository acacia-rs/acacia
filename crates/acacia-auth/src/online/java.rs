//! Java Edition sign-in: an XSTS token for Minecraft services becomes a Java access token and profile.

use reqwest::header::CONTENT_TYPE;
use serde::{Deserialize, Serialize};
use serde_json::json;

use super::account::Account;
use super::client::AuthClient;
use super::http::Http;
use super::tokens::CachedTokens;
use super::xbox::{self, XboxToken};
use crate::jwt::now_unix;
use crate::{Error, Result};

pub const JAVA_RELYING_PARTY: &str = "rp://api.minecraftservices.com/";
const LOGIN: &str = "https://api.minecraftservices.com/authentication/login_with_xbox";
const PROFILE: &str = "https://api.minecraftservices.com/minecraft/profile";
const JOIN: &str = "https://sessionserver.mojang.com/session/minecraft/join";

/// A signed-in Java Edition player.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct JavaProfile {
    pub access_token: String,
    /// Undashed, as the session server wants it.
    pub uuid: String,
    pub name: String,
    /// Unix seconds.
    pub expires_at: i64,
}

impl JavaProfile {
    fn is_valid(&self) -> bool {
        self.expires_at - 60 > now_unix()
    }
}

#[derive(Deserialize)]
struct Login {
    access_token: String,
    expires_in: i64,
}

#[derive(Deserialize)]
struct Profile {
    id: String,
    name: String,
}

async fn profile(http: &Http, xsts: &XboxToken) -> Result<JavaProfile> {
    let body = json!({ "identityToken": xsts.authorization() }).to_string();
    let login: Login = http.json(http.client.post(LOGIN).header(CONTENT_TYPE, "application/json").body(body), LOGIN).await?;
    let p: Profile = http.json(http.client.get(PROFILE).bearer_auth(&login.access_token), PROFILE).await?;
    Ok(JavaProfile { access_token: login.access_token, uuid: p.id, name: p.name, expires_at: now_unix() + login.expires_in })
}

async fn java_xsts(client: &AuthClient, state: &mut CachedTokens) -> Result<XboxToken> {
    if let Some(t) = state.xsts.get(JAVA_RELYING_PARTY).filter(|t| t.is_valid()) {
        return Ok(t.clone());
    }
    let title = client.config().title;
    let msa = client.valid_msa(state).await?;
    let key = state.xbox_key();
    let device = client.valid_device(state, &key).await?;
    let (title_token, user) = xbox::live_tokens(client.http(), title, &key, &msa.access_token, &device).await?;
    let t = xbox::xsts(client.http(), title, &key, JAVA_RELYING_PARTY, &device, &title_token, &user).await?;
    state.xsts.insert(JAVA_RELYING_PARTY.to_owned(), t.clone());
    Ok(t)
}

impl Account {
    /// The account's Java profile. The account must be signed in under [`Title::NintendoSwitch`]: Minecraft services
    /// refuses the Android title with HTTP 403. Fails with HTTP 404 from the profile endpoint without Java Edition.
    ///
    /// [`Title::NintendoSwitch`]: super::Title::NintendoSwitch
    pub async fn java_profile(&self) -> Result<JavaProfile> {
        self.with_tokens(async |t| {
            if let Some(p) = t.java_profile.clone().filter(JavaProfile::is_valid) {
                return Ok(p);
            }
            let xsts = java_xsts(self.client(), t).await?;
            let p = profile(self.client().http(), &xsts).await?;
            t.java_profile = Some(p.clone());
            Ok(p)
        })
        .await
    }

    /// Tells the session server this player is joining the server identified by `server_hash`.
    pub async fn join_java_session(&self, player: &JavaProfile, server_hash: &str) -> Result<()> {
        let body = json!({ "accessToken": player.access_token, "selectedProfile": player.uuid, "serverId": server_hash }).to_string();
        let resp = self.client().http().client.post(JOIN).header(CONTENT_TYPE, "application/json").body(body).send().await?;
        let status = resp.status();
        if status.is_success() {
            return Ok(());
        }
        Err(Error::Status { endpoint: JOIN.into(), status: status.as_u16(), body: resp.text().await.unwrap_or_default() })
    }
}
