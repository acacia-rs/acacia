//! Joining a [`Target::Java`](super::spec::Target::Java) server: the swarm's policy, leases and shards apply as on
//! Bedrock, but Java bots share no worlds and take no proxy or client hook.

use acacia_client::Socks5Proxy;

use super::login::{Failure, NodeAuth};
use super::spec::Login;
use crate::{Bot, BotConfig};

#[cfg(feature = "java")]
pub(crate) async fn connect(login: &Login, address: &str, proxy: Option<&Socks5Proxy>, auth: Option<&NodeAuth>, config: BotConfig) -> Result<Bot, Failure> {
    if proxy.is_some() {
        return Err(Failure::Invalid("Java targets take no proxy".into()));
    }
    let (host, port) = match address.rsplit_once(':') {
        Some((host, port)) => (host, port.parse().map_err(|_| Failure::Invalid(format!("bad Java address {address}")))?),
        None => (address, JAVA_PORT),
    };
    let joined = match login {
        Login::Offline { name } => Bot::connect_java_offline(host, port, name, config).await,
        Login::Online { account } => {
            let auth = auth.ok_or_else(|| Failure::Invalid("online login but the swarm has no token cache".into()))?;
            let (_, account) = auth.account(account, None, true).await?;
            Bot::connect_java(host, port, &account, config).await
        }
    };
    joined.map_err(|e| match e {
        acacia_java_client::Error::Auth(e) => Failure::Auth(e),
        e => Failure::Session(crate::bot::java_disconnect_reason(e)),
    })
}

#[cfg(feature = "java")]
const JAVA_PORT: u16 = 25565;

#[cfg(not(feature = "java"))]
pub(crate) async fn connect(_: &Login, _: &str, _: Option<&Socks5Proxy>, _: Option<&NodeAuth>, _: BotConfig) -> Result<Bot, Failure> {
    Err(Failure::Invalid("Java targets need acacia-bot's `java` feature".into()))
}
