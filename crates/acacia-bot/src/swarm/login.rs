use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use acacia_client::auth::{Account, AuthClient, AuthConfig, Error as AuthError, TokenCache};
use acacia_client::{realm_builder, Client, ClientBuilder, ConnectError, RealmJoinError, Socks5Proxy};

use super::policy::Ended;
use super::spec::{Login, Target};

/// The node's sign-in side: one token cache (share it across nodes for horizontal scaling) and an
/// auth client per proxy, so each bot signs in through the same proxy it plays through.
pub(crate) struct NodeAuth {
    config: AuthConfig,
    cache: Arc<dyn TokenCache>,
    clients: Mutex<HashMap<Option<String>, Arc<AuthClient>>>,
}

pub(crate) enum Failure {
    Auth(AuthError),
    Connect(ConnectError),
    Invalid(String),
}

impl Failure {
    pub fn ended(&self) -> Ended<'_> {
        match self {
            Failure::Auth(e) => Ended::Auth(e),
            Failure::Connect(e) => Ended::Connect(e),
            Failure::Invalid(e) => Ended::Invalid(e.clone()),
        }
    }
}

impl From<RealmJoinError> for Failure {
    fn from(e: RealmJoinError) -> Self {
        match e {
            RealmJoinError::Auth(e) => Failure::Auth(e),
            RealmJoinError::Connect(e) => Failure::Connect(e),
            e @ RealmJoinError::Unsupported(_) => Failure::Invalid(e.to_string()),
        }
    }
}

impl NodeAuth {
    pub fn new(config: AuthConfig, cache: Arc<dyn TokenCache>) -> Self {
        Self { config, cache, clients: Mutex::new(HashMap::new()) }
    }

    fn account(&self, id: &str, proxy: Option<&Socks5Proxy>) -> Result<(Arc<AuthClient>, Account), Failure> {
        let url = proxy.map(Socks5Proxy::to_url);
        let mut clients = self.clients.lock().expect("auth clients lock poisoned");
        let client = match clients.get(&url) {
            Some(c) => c.clone(),
            None => {
                let config = AuthConfig { proxy: url.clone(), ..self.config.clone() };
                // Only a bad proxy URL (e.g. socks5 without the `socks` feature) fails here.
                let c = Arc::new(AuthClient::new(config).map_err(|e| Failure::Invalid(e.to_string()))?);
                clients.insert(url, c.clone());
                c
            }
        };
        let account = Account::new(client.clone(), self.cache.clone(), id);
        if !account.is_signed_in() {
            return Err(Failure::Invalid(format!("account {id} is not signed in")));
        }
        Ok((client, account))
    }
}

/// A builder for `target`, signed in as `login`.
pub(crate) async fn client_builder(
    login: &Login,
    target: &Target,
    proxy: Option<&Socks5Proxy>,
    auth: Option<&NodeAuth>,
) -> Result<ClientBuilder, Failure> {
    let builder = match login {
        Login::Offline { name } => match target {
            Target::Server { address } => Client::builder(address).offline(name),
            Target::Realm { .. } => return Err(Failure::Invalid("realms need an online login".into())),
        },
        Login::Online { account } => {
            let auth = auth.ok_or_else(|| Failure::Invalid("online login but the swarm has no token cache".into()))?;
            let (client, account) = auth.account(account, proxy)?;
            match target {
                Target::Server { address } => {
                    let (key, credentials) = account.login_credentials().await.map_err(Failure::Auth)?;
                    Client::builder(address).online(credentials, key)
                }
                Target::Realm { id } => realm_builder(&client, &account, *id, proxy).await?,
            }
        }
    };
    Ok(match proxy {
        Some(p) => builder.proxy(p.clone()),
        None => builder,
    })
}
