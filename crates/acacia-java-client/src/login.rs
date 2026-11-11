//! Login and the first configuration, on the bare connection before [`crate::link::Link`] takes it over.

use acacia_auth::{Account, JavaProfile};
use acacia_java_proto::packets::{CookieRequest, CookieResponse, CustomQuery, CustomQueryAnswer, Empty, EncryptionHello, EncryptionKey, LoginCompression, LoginFinished};
use acacia_java_proto::{Wire, ids};
use md5::{Digest, Md5};

use crate::configuration::{self, Cookies, Registries};
use crate::crypto::{random_secret, rsa_encrypt, server_hash};
use crate::frame::{Connection, Error};

/// The id an offline-mode server gives `name`: a v3 UUID of `OfflinePlayer:<name>`.
pub fn offline_uuid(name: &str) -> u128 {
    let mut hash: [u8; 16] = Md5::digest(format!("OfflinePlayer:{name}")).into();
    hash[6] = hash[6] & 0x0f | 0x30;
    hash[8] = hash[8] & 0x3f | 0x80;
    u128::from_be_bytes(hash)
}

/// The account and profile to authenticate with; `None` joins offline.
pub(crate) type Session<'a> = Option<(&'a Account, &'a JavaProfile)>;

/// Answers the server's encryption request and turns encryption on.
async fn encrypt(c: &mut Connection, hello: EncryptionHello, session: Session<'_>) -> Result<(), Error> {
    let secret = random_secret();
    if hello.should_authenticate {
        let (account, player) = session.ok_or(Error::NeedsAccount)?;
        account.join_java_session(player, &server_hash(&hello.server_id, &secret, &hello.public_key)).await?;
    }
    let key = EncryptionKey { secret: rsa_encrypt(&hello.public_key, &secret)?, challenge: rsa_encrypt(&hello.public_key, &hello.challenge)? };
    c.send(ids::login::serverbound::KEY, &key).await?;
    c.enable_encryption(&secret);
    Ok(())
}

/// Logs in, then sends what a client sends on entering configuration ([`configuration::hello`]).
pub(crate) async fn login(c: &mut Connection, session: Session<'_>) -> Result<LoginFinished, Error> {
    use ids::login::{clientbound as from, serverbound as to};
    loop {
        let (id, body) = c.read().await?;
        let mut input = &body[..];
        match id {
            from::HELLO => encrypt(c, EncryptionHello::read(&mut input)?, session).await?,
            from::LOGIN_COMPRESSION => c.set_compression(LoginCompression::read(&mut input)?.threshold.0),
            // A fresh connection has no cookies (only a transfer carries them over).
            from::COOKIE_REQUEST => c.send(to::COOKIE_RESPONSE, &CookieResponse { key: CookieRequest::read(&mut input)?.key, payload: None }).await?,
            // Login plugin queries go unanswered by a vanilla client: "no payload".
            from::CUSTOM_QUERY => c.send(to::CUSTOM_QUERY_ANSWER, &CustomQueryAnswer { transaction: CustomQuery::read(&mut input)?.transaction, payload: None }).await?,
            from::LOGIN_FINISHED => {
                let profile = LoginFinished::read(&mut input)?;
                c.send(to::LOGIN_ACKNOWLEDGED, &Empty {}).await?;
                for packet in configuration::hello() {
                    c.write(packet).await?;
                }
                return Ok(profile);
            }
            from::LOGIN_DISCONNECT => return Err(Error::Disconnected(String::read(&mut input)?)),
            other => return Err(Error::Unexpected(other, "login")),
        }
    }
}

/// The first configuration phase, up to its `FinishConfiguration`.
pub(crate) async fn configure(c: &mut Connection, cookies: &mut Cookies) -> Result<Registries, Error> {
    use ids::configuration::clientbound as from;
    let mut registries = Registries::default();
    loop {
        let (id, body) = c.read().await?;
        if id == from::DISCONNECT {
            return Err(Error::Disconnected(format!("{body:02x?}")));
        }
        registries.collect(id, &body)?;
        for packet in configuration::answer(id, &body, cookies)? {
            c.write(packet).await?;
        }
        if id == from::FINISH_CONFIGURATION {
            return Ok(registries);
        }
    }
}
