use std::sync::Arc;

use p384::ecdsa::SigningKey;

use super::cache::{Stored, TokenCache, Versioned};
use super::client::AuthClient;
use super::live::{DeviceCodePrompt, MsaToken};
use super::minecraft::ServiceToken;
use super::realms::{self, PingRegion, Realm, RealmJoin, REALMS_RELYING_PARTY};
use super::tokens::CachedTokens;
use super::xbox::XboxToken;
use crate::{LoginCredentials, Result};

/// One Microsoft account backed by a [`TokenCache`]: reuses every cached token while it is
/// valid and writes back after each refresh (also after a failed login, so partial progress
/// such as a new device token is kept). If another writer got there first, its copy wins.
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
    pub async fn is_signed_in(&self) -> Result<bool> {
        Ok(self.cache.load(&self.id).await?.is_some_and(|v| v.tokens.msa.is_some()))
    }

    /// Runs the device code flow: `on_prompt` shows the code to the user, then this waits for
    /// the sign-in and stores the MSA token. Existing device key and device token are kept.
    pub async fn sign_in(&self, on_prompt: impl FnOnce(&DeviceCodePrompt)) -> Result<MsaToken> {
        let prompt = self.client.start_device_code().await?;
        on_prompt(&prompt);
        let msa = self.client.poll_device_code(&prompt).await?;
        loop {
            let read = self.load().await?;
            let mut tokens = read.tokens;
            tokens.msa = Some(msa.clone());
            tokens.reset_user_tokens();
            if self.cache.store(&self.id, &tokens, Some(read.version)).await? != Stored::Conflict {
                return Ok(msa);
            }
        }
    }

    /// Valid credentials for `client_key`, refreshing only what has expired.
    pub async fn credentials(&self, client_key: &SigningKey) -> Result<LoginCredentials> {
        self.with_tokens(async |t| self.client.credentials_with(t, client_key).await).await
    }

    /// The account's persisted client key and credentials bound to it. Prefer this for
    /// reconnects: unexpired credentials are reused instead of hitting rate-limited endpoints.
    pub async fn login_credentials(&self) -> Result<(SigningKey, LoginCredentials)> {
        self.with_tokens(async |t| {
            let key = t.client_key();
            let credentials = self.client.credentials_with(t, &key).await?;
            Ok((key, credentials))
        })
        .await
    }

    /// An XSTS token for another Xbox service, e.g. Realms (`https://pocket.realms.minecraft.net/`).
    pub async fn xsts_token(&self, relying_party: &str) -> Result<XboxToken> {
        self.with_tokens(async |t| self.client.xsts_with(t, relying_party).await).await
    }

    /// The Minecraft services token (`MCToken`), e.g. for the NetherNet signaling service.
    pub async fn service_token(&self) -> Result<ServiceToken> {
        self.with_tokens(async |t| self.client.service_token_with(t).await).await
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

    /// Runs `f` on the cached tokens and writes back whatever it refreshed, even if it failed.
    async fn with_tokens<T>(&self, f: impl AsyncFnOnce(&mut CachedTokens) -> Result<T>) -> Result<T> {
        let read = self.load().await?;
        let mut tokens = read.tokens.clone();
        let result = f(&mut tokens).await;
        if tokens != read.tokens && self.cache.store(&self.id, &tokens, Some(read.version)).await? == Stored::Conflict {
            tracing::debug!(account = %self.id, "tokens were refreshed concurrently; keeping the stored copy");
        }
        result
    }

    /// The cached tokens, creating (and storing) a fresh device identity on first use.
    async fn load(&self) -> Result<Versioned> {
        loop {
            if let Some(read) = self.cache.load(&self.id).await? {
                return Ok(read);
            }
            let tokens = CachedTokens::new(None);
            if let Stored::Written { version } = self.cache.store(&self.id, &tokens, None).await? {
                return Ok(Versioned { tokens, version });
            }
        }
    }
}
