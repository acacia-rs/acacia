//! Microsoft sign-in for `createBot {auth: microsoft}` and `signIn` (contract: the crate docs, "Sign-in").

#[cfg(feature = "fake-sign-in")]
mod fake;
mod microsoft;
mod runner;

use std::fmt;
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;

use acacia_api::{ApiError, ErrorCode};
use acacia_client::auth::{Account, LoginCredentials};
use p384::ecdsa::SigningKey;

#[cfg(feature = "fake-sign-in")]
pub use fake::{FakeSignIn, FAKE_USER_CODE};
pub use microsoft::MicrosoftSignIn;
pub(crate) use runner::{msa_code, SignIns};

/// The account a sign-in is for: its key in the token cache, and the cache's directory.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MsaAccount {
    pub username: String,
    pub profiles_folder: PathBuf,
}

/// The device code a person must enter at `verification_uri`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceCode {
    pub user_code: String,
    pub verification_uri: String,
    /// Seconds the code stays valid.
    pub expires_in: u64,
}

/// Shows a [`DeviceCode`] to a person; called at most once per sign-in.
pub type Prompt = Box<dyn FnOnce(DeviceCode) + Send>;

/// What an online bot logs in with: credentials bound to `key`.
pub struct OnlineLogin {
    pub key: SigningKey,
    pub credentials: LoginCredentials,
}

pub type LoginFuture = Pin<Box<dyn Future<Output = Result<OnlineLogin, ApiError>> + Send>>;
pub type JavaFuture = Pin<Box<dyn Future<Output = Result<Account, ApiError>> + Send>>;

/// Where Microsoft sign-in goes ([`crate::HostConfig::sign_in`]): [`MicrosoftSignIn`] unless a test swaps in a fake.
/// Runs on the host's sign-in thread, never on a shard.
pub trait SignIn: Send + Sync {
    /// Credentials for `account`, calling `prompt` when no cached sign-in will do; fails `AUTH`.
    fn login(&self, account: MsaAccount, prompt: Prompt) -> LoginFuture;

    /// `account` signed in with a Java profile, calling `prompt` when no cached sign-in will do; fails `AUTH`.
    fn java_account(&self, account: MsaAccount, prompt: Prompt) -> JavaFuture {
        drop((account, prompt));
        Box::pin(async { Err(ApiError::new(ErrorCode::Auth, "this sign-in has no Java accounts")) })
    }
}

impl fmt::Debug for dyn SignIn {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("dyn SignIn")
    }
}

/// `~/.acacia/auth` of the user running the host (the current directory's `.acacia/auth` without a home).
pub fn default_profiles_folder() -> PathBuf {
    std::env::home_dir().unwrap_or_default().join(".acacia").join("auth")
}
