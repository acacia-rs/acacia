use crate::auth::{Account, AuthClient, RealmProtocol};
use crate::{measure_ping_regions, Client, ClientBuilder, ConnectError, SignalingTarget, Socks5Proxy};

const DEFAULT_SIGNALING_HOST: &str = "signal.franchise.minecraft-services.net";

#[derive(Debug, thiserror::Error)]
pub enum RealmJoinError {
    #[error(transparent)]
    Auth(#[from] crate::auth::Error),
    #[error(transparent)]
    Connect(#[from] ConnectError),
    #[error("unsupported realm protocol {0:?}")]
    Unsupported(RealmProtocol),
}

/// Asks Realms to start `realm_id` and returns a signed-in builder pointed at it: RakNet realms
/// dial the returned address, NetherNet realms go through the signaling service.
pub async fn realm_builder(
    auth: &AuthClient,
    account: &Account,
    realm_id: i64,
    proxy: Option<&Socks5Proxy>,
) -> Result<ClientBuilder, RealmJoinError> {
    let route = realm_route(auth, account, realm_id, proxy).await?;
    let (key, credentials) = account.login_credentials().await?;
    Ok(match route {
        RealmRoute::Address(address) => Client::builder(address).online(credentials, key),
        RealmRoute::Signaled(target) => Client::builder(target.peer.clone()).online(credentials, key).signaling(target),
    })
}

/// How a started realm is reached.
#[derive(Debug, Clone)]
pub enum RealmRoute {
    /// A RakNet realm's `host:port`.
    Address(String),
    /// A NetherNet realm, through the signaling service.
    Signaled(SignalingTarget),
}

/// Asks Realms to start `realm_id` and says how to reach it, for a caller that dials it itself
/// ([`crate::RawLink`]); [`realm_builder`] is this plus a signed-in builder.
pub async fn realm_route(
    auth: &AuthClient,
    account: &Account,
    realm_id: i64,
    proxy: Option<&Socks5Proxy>,
) -> Result<RealmRoute, RealmJoinError> {
    let ping_regions = measure_ping_regions(&auth.qos_beacons().await?, proxy).await?;
    let join = account.join_realm(realm_id, &ping_regions).await?;
    if join.protocol == RealmProtocol::RakNet {
        return Ok(RealmRoute::Address(join.address));
    }
    let token = account.service_token().await?.authorization_header;
    let host = signaling_host(auth).await?;
    let target = SignalingTarget::from_realm(&join, token, &host).ok_or(RealmJoinError::Unsupported(join.protocol))?;
    Ok(RealmRoute::Signaled(target))
}

/// Discovery's signaling service host (what vanilla dials for realms and friends' worlds).
pub(crate) async fn signaling_host(auth: &AuthClient) -> Result<String, crate::auth::Error> {
    Ok(auth.signaling_environment().await?.map_or_else(|| DEFAULT_SIGNALING_HOST.to_owned(), |env| env.service_uri))
}
