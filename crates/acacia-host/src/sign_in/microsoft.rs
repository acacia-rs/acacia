use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use acacia_api::{ApiError, ErrorCode};
use acacia_client::auth::{Account, AuthClient, AuthConfig, DeviceCodePrompt, Error as AuthError, FileTokenCache};
use parking_lot::Mutex;

use super::{DeviceCode, JavaFuture, LoginFuture, MsaAccount, OnlineLogin, Prompt, SignIn};

/// acacia-auth's device-code flow (docs/auth.md) with one [`FileTokenCache`] per profiles folder: a cache's
/// versions only hold within one instance, so every bot of the host shares it.
pub struct MicrosoftSignIn {
    config: AuthConfig,
    client: Mutex<Option<Arc<AuthClient>>>,
    caches: Mutex<HashMap<PathBuf, Arc<FileTokenCache>>>,
}

impl MicrosoftSignIn {
    pub fn new(config: AuthConfig) -> Self {
        Self { config, client: Mutex::new(None), caches: Mutex::new(HashMap::new()) }
    }

    fn account(&self, account: &MsaAccount) -> Result<Account, ApiError> {
        let client = match &mut *self.client.lock() {
            Some(client) => client.clone(),
            slot => slot.insert(Arc::new(AuthClient::new(self.config.clone()).map_err(auth)?)).clone(),
        };
        let cache = self.cache(&account.profiles_folder)?;
        Ok(Account::new(client, cache, account.username.clone()))
    }

    fn cache(&self, folder: &Path) -> Result<Arc<FileTokenCache>, ApiError> {
        let mut caches = self.caches.lock();
        if let Some(cache) = caches.get(folder) {
            return Ok(cache.clone());
        }
        let cache = FileTokenCache::new(folder)
            .map_err(|e| ApiError::new(ErrorCode::Auth, format!("token cache {}: {e}", folder.display())))?;
        Ok(caches.entry(folder.to_owned()).or_insert(Arc::new(cache)).clone())
    }
}

impl Default for MicrosoftSignIn {
    fn default() -> Self {
        Self::new(AuthConfig::default())
    }
}

impl SignIn for MicrosoftSignIn {
    fn login(&self, account: MsaAccount, prompt: Prompt) -> LoginFuture {
        let account = self.account(&account);
        Box::pin(async move {
            let account = account?;
            let mut prompt = Some(prompt);
            if !account.is_signed_in().await.map_err(auth)? {
                sign_in(&account, prompt.take()).await?;
            }
            let (key, credentials) = match account.login_credentials().await {
                // A refresh token past its life: the cached sign-in is useless, so sign in again.
                Err(AuthError::OAuth { .. }) if prompt.is_some() => {
                    sign_in(&account, prompt.take()).await?;
                    account.login_credentials().await.map_err(auth)?
                }
                got => got.map_err(auth)?,
            };
            Ok(OnlineLogin { key, credentials })
        })
    }

    fn java_account(&self, account: MsaAccount, prompt: Prompt) -> JavaFuture {
        let account = self.account(&account);
        Box::pin(async move {
            let account = account?;
            let mut prompt = Some(prompt);
            if !account.is_signed_in().await.map_err(auth)? {
                sign_in(&account, prompt.take()).await?;
            }
            match account.java_profile().await {
                Err(AuthError::OAuth { .. }) if prompt.is_some() => {
                    sign_in(&account, prompt.take()).await?;
                    account.java_profile().await.map_err(auth)?;
                }
                got => drop(got.map_err(auth)?),
            }
            Ok(account)
        })
    }
}

async fn sign_in(account: &Account, prompt: Option<Prompt>) -> Result<(), ApiError> {
    let prompt = prompt.expect("one sign-in per login");
    account.sign_in(move |p: &DeviceCodePrompt| prompt(device_code(p))).await.map(drop).map_err(auth)
}

fn device_code(p: &DeviceCodePrompt) -> DeviceCode {
    DeviceCode { user_code: p.user_code.clone(), verification_uri: p.verification_uri.clone(), expires_in: p.expires_in }
}

fn auth(e: AuthError) -> ApiError {
    ApiError::new(ErrorCode::Auth, e.to_string())
}
