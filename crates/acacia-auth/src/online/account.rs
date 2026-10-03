use std::sync::Arc;

use p384::ecdsa::SigningKey;

use super::cache::{CachedTokens, TokenCache};
use super::client::AuthClient;
use super::live::{DeviceCodePrompt, MsaToken};
use super::minecraft::ServiceToken;
use super::realms::{self, PingRegion, Realm, RealmJoin, REALMS_RELYING_PARTY};
use super::xbox::XboxToken;
use crate::{LoginCredentials, Result};

/// One Microsoft account backed by a [`TokenCache`]: reuses every cached token while it is
/// valid and writes back after each refresh (also after a failed login, so partial progress
/// such as a new device token is kept).
pub struct Account {
    client: Arc<AuthClient>,
    cache: Arc<dyn TokenCache>,
    id: String,
}

impl Account {
    pub fn new(client: Arc<AuthClient>, cache: Arc<dyn TokenCache>, id: impl Into<String>) -> Self {
        Self { client, cache, id: id.into() }
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    /// True if the cache holds an MSA token, i.e. no interactive login is needed.
    pub fn is_signed_in(&self) -> bool {
        self.cache.load(&self.id).is_some_and(|t| t.msa.is_some())
    }

    /// Runs the device code flow: `on_prompt` shows the code to the user, then this waits for
    /// the sign-in and stores the MSA token. Existing device key and device token are kept.
    pub async fn sign_in(&self, on_prompt: impl FnOnce(&DeviceCodePrompt)) -> Result<MsaToken> {
        let prompt = self.client.start_device_code().await?;
        on_prompt(&prompt);
        let msa = self.client.poll_device_code(&prompt).await?;
        let mut tokens = self.load();
        tokens.msa = Some(msa.clone());
        tokens.reset_user_tokens();
        self.cache.store(&self.id, &tokens);
        Ok(msa)
    }

    /// Valid credentials for `client_key`, refreshing only what has expired.
    pub async fn credentials(&self, client_key: &SigningKey) -> Result<LoginCredentials> {
        let mut tokens = self.load();
        self.refresh(&mut tokens, client_key).await
    }

    /// The account's persisted client key and credentials bound to it. Prefer this for
    /// reconnects: unexpired credentials are reused instead of hitting rate-limited endpoints.
    pub async fn login_credentials(&self) -> Result<(SigningKey, LoginCredentials)> {
        let mut tokens = self.load();
        let key = tokens.client_key();
        let credentials = self.refresh(&mut tokens, &key).await?;
        Ok((key, credentials))
    }

    /// An XSTS token for another Xbox service, e.g. Realms (`https://pocket.realms.minecraft.net/`).
    pub async fn xsts_token(&self, relying_party: &str) -> Result<XboxToken> {
        let mut tokens = self.load();
        let before = tokens.clone();
        let result = self.client.xsts_with(&mut tokens, relying_party).await;
        self.store_if_changed(&tokens, &before);
        result
    }

    /// The Minecraft services token (`MCToken`), e.g. for the NetherNet signaling service.
    pub async fn service_token(&self) -> Result<ServiceToken> {
        let mut tokens = self.load();
        let before = tokens.clone();
        let result = self.client.service_token_with(&mut tokens).await;
        self.store_if_changed(&tokens, &before);
        result
    }

    /// Realms the account owns or is a member of.
    pub async fn realms(&self) -> Result<Vec<Realm>> {
        realms::list(&self.client, &self.xsts_token(REALMS_RELYING_PARTY).await?).await
    }

    /// The realm behind an invite code or `realms.gg` link, without joining it.
    pub async fn realm_by_invite(&self, code: &str) -> Result<Realm> {
        realms::by_invite(&self.client, &self.xsts_token(REALMS_RELYING_PARTY).await?, code).await
    }

    /// Accepts an invite code, making the account a member.
    pub async fn accept_realm_invite(&self, code: &str) -> Result<Realm> {
        realms::accept_invite(&self.client, &self.xsts_token(REALMS_RELYING_PARTY).await?, code).await
    }

    /// Where and how to connect to a realm, waiting while it starts. See [`PingRegion`].
    pub async fn join_realm(&self, realm_id: i64, ping_regions: &[PingRegion]) -> Result<RealmJoin> {
        realms::join(&self.client, &self.xsts_token(REALMS_RELYING_PARTY).await?, realm_id, ping_regions).await
    }

    async fn refresh(&self, tokens: &mut CachedTokens, key: &SigningKey) -> Result<LoginCredentials> {
        let before = tokens.clone();
        let result = self.client.credentials_with(tokens, key).await;
        self.store_if_changed(tokens, &before);
        result
    }

    fn store_if_changed(&self, tokens: &CachedTokens, before: &CachedTokens) {
        if tokens != before {
            self.cache.store(&self.id, tokens);
        }
    }

    fn load(&self) -> CachedTokens {
        self.cache.load(&self.id).unwrap_or_else(|| {
            let fresh = CachedTokens::new(None);
            self.cache.store(&self.id, &fresh);
            fresh
        })
    }
}
