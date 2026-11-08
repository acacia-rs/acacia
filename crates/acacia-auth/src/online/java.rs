//! Java Edition sign-in: an XSTS token for Minecraft services becomes a Java access token and profile.

use reqwest::header::CONTENT_TYPE;
use serde::Deserialize;
use serde_json::json;

use super::account::Account;
use super::http::Http;
use super::xbox::XboxToken;
use crate::{Error, Result};

pub const JAVA_RELYING_PARTY: &str = "rp://api.minecraftservices.com/";
const LOGIN: &str = "https://api.minecraftservices.com/authentication/login_with_xbox";
const PROFILE: &str = "https://api.minecraftservices.com/minecraft/profile";
const JOIN: &str = "https://sessionserver.mojang.com/session/minecraft/join";

/// A signed-in Java Edition player.
#[derive(Debug, Clone)]
pub struct JavaProfile {
    pub access_token: String,
    /// Undashed, as the session server wants it.
    pub uuid: String,
    pub name: String,
}

#[derive(Deserialize)]
struct Login {
    access_token: String,
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
    Ok(JavaProfile { access_token: login.access_token, uuid: p.id, name: p.name })
}

impl Account {
    /// The account's Java profile. Fails with HTTP 404 from the profile endpoint if it doesn't own Java Edition.
    pub async fn java_profile(&self) -> Result<JavaProfile> {
        let xsts = self.xsts_token(JAVA_RELYING_PARTY).await?;
        profile(self.http(), &xsts).await
    }

    /// Tells the session server this player is joining the server identified by `server_hash`.
    pub async fn join_java_session(&self, player: &JavaProfile, server_hash: &str) -> Result<()> {
        let body = json!({ "accessToken": player.access_token, "selectedProfile": player.uuid, "serverId": server_hash }).to_string();
        let resp = self.http().client.post(JOIN).header(CONTENT_TYPE, "application/json").body(body).send().await?;
        let status = resp.status();
        if status.is_success() {
            return Ok(());
        }
        Err(Error::Status { endpoint: JOIN.into(), status: status.as_u16(), body: resp.text().await.unwrap_or_default() })
    }
}
