//! Fetches the multiplayer token issuer's signing keys for `login::verify` (servers).

use serde::Deserialize;

use super::client::AuthClient;
use super::config::endpoints as ep;
use crate::login::verify::{MULTIPLAYER_ISSUER, SigningKeys};
use crate::{Error, Result};

#[derive(Deserialize)]
struct OpenIdConfiguration {
    issuer: String,
    jwks_uri: String,
}

impl AuthClient {
    /// The issuer's current JWKS, located through its OpenID configuration
    /// (`<issuer>.well-known/openid-configuration` → `jwks_uri`). Keys rotate: call again when a
    /// login fails with `VerifyError::UnknownSigningKey`, rate-limited (gophertunnel: 30 min).
    pub async fn fetch_signing_keys(&self) -> Result<SigningKeys> {
        let config_url = format!("{MULTIPLAYER_ISSUER}.well-known/openid-configuration");
        let config: OpenIdConfiguration = self.http().json(self.services_get(&config_url), &config_url).await?;
        if config.issuer != MULTIPLAYER_ISSUER || !config.jwks_uri.starts_with(MULTIPLAYER_ISSUER) {
            let reason = format!("issuer {:?} serves keys at {:?}", config.issuer, config.jwks_uri);
            return Err(Error::Protocol { endpoint: config_url, reason });
        }
        let jwks = self.http().send(self.services_get(&config.jwks_uri), &config.jwks_uri).await?.text().await?;
        Ok(SigningKeys::from_jwks(&jwks)?)
    }

    fn services_get(&self, url: &str) -> reqwest::RequestBuilder {
        self.http().client.get(url).header("Accept", "application/json").header("User-Agent", ep::SERVICES_USER_AGENT)
    }
}
