use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use acacia_client::auth::LoginCredentials;
use p384::ecdsa::SigningKey;
use parking_lot::Mutex;

use super::{DeviceCode, LoginFuture, MsaAccount, OnlineLogin, Prompt, SignIn};

/// The code [`FakeSignIn`] prompts with.
pub const FAKE_USER_CODE: &str = "FAKE1234";

/// Signs in without Microsoft, for tests: prompts with [`FAKE_USER_CODE`] the first time an account (and folder) signs
/// in, waits `delay` as a person would, then hands out unsigned online credentials that only a server not checking
/// them (acacia-testserver) takes. Nothing touches the disk.
pub struct FakeSignIn {
    delay: Duration,
    signed_in: Arc<Mutex<HashSet<MsaAccount>>>,
}

impl FakeSignIn {
    pub fn new(delay: Duration) -> Self {
        Self { delay, signed_in: Arc::default() }
    }
}

impl SignIn for FakeSignIn {
    fn login(&self, account: MsaAccount, prompt: Prompt) -> LoginFuture {
        let (delay, signed_in) = (self.delay, self.signed_in.clone());
        Box::pin(async move {
            if !signed_in.lock().contains(&account) {
                let uri = "https://www.microsoft.com/link".to_owned();
                prompt(DeviceCode { user_code: FAKE_USER_CODE.into(), verification_uri: uri, expires_in: 900 });
                tokio::time::sleep(delay).await;
                signed_in.lock().insert(account.clone());
            }
            let credentials = LoginCredentials {
                chain: Vec::new(),
                multiplayer_token: Some("fake.multiplayer.token".into()),
                xuid: "2535400000000001".into(),
                display_name: account.username,
                identity: "00000000-0000-4000-8000-000000000001".into(),
                playfab_id: None,
                expires_at: i64::MAX,
            };
            Ok(OnlineLogin { key: SigningKey::from_slice(&[7; 48]).expect("a valid scalar"), credentials })
        })
    }
}
