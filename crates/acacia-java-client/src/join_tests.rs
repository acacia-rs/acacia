//! Encrypted login against a local stand-in: vanilla only encrypts in online mode, which needs a real account.

use acacia_java_proto::packets::{EncryptionHello, EncryptionKey, LoginFinished, LoginHello};
use acacia_java_proto::{Wire, ids};
use rsa::pkcs8::EncodePublicKey;
use rsa::rand_core::OsRng;
use rsa::{Pkcs1v15Encrypt, RsaPrivateKey};
use tokio::net::{TcpListener, TcpStream};

use super::*;

async fn serve(stream: TcpStream) -> u128 {
    let mut c = Connection::new(stream);
    c.read().await.unwrap();
    let (_, hello) = c.read().await.unwrap();
    let uuid = LoginHello::read(&mut &hello[..]).unwrap().uuid;

    let private = RsaPrivateKey::new(&mut OsRng, 1024).unwrap();
    let public_key = private.to_public_key().to_public_key_der().unwrap().as_bytes().to_vec();
    let challenge = vec![1, 2, 3, 4];
    c.send(ids::login::clientbound::HELLO, &EncryptionHello { server_id: String::new(), public_key, challenge: challenge.clone(), should_authenticate: false }).await.unwrap();

    let (id, body) = c.read().await.unwrap();
    assert_eq!(id, ids::login::serverbound::KEY);
    let key = EncryptionKey::read(&mut &body[..]).unwrap();
    assert_eq!(private.decrypt(Pkcs1v15Encrypt, &key.challenge).unwrap(), challenge);
    let secret: [u8; 16] = private.decrypt(Pkcs1v15Encrypt, &key.secret).unwrap().try_into().unwrap();
    c.enable_encryption(&secret);

    c.send(ids::login::clientbound::LOGIN_FINISHED, &LoginFinished { uuid, name: "Enc".into() }).await.unwrap();
    assert_eq!(c.read().await.unwrap().0, ids::login::serverbound::LOGIN_ACKNOWLEDGED);
    c.send(ids::configuration::clientbound::FINISH_CONFIGURATION, &Empty {}).await.unwrap();
    assert_eq!(c.read().await.unwrap().0, ids::configuration::serverbound::FINISH_CONFIGURATION);
    c.send(ids::play::clientbound::PING, &Ping { id: 77 }).await.unwrap();
    let (id, body) = c.read().await.unwrap();
    assert_eq!((id, Ping::read(&mut &body[..]).unwrap().id), (ids::play::serverbound::PONG, 77));
    uuid
}

#[tokio::test]
async fn login_continues_encrypted_after_the_key_exchange() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = tokio::spawn(async move { serve(listener.accept().await.unwrap().0).await });
    let mut client = Client::join_offline("127.0.0.1", port, "Enc").await.unwrap();
    assert_eq!(client.name, "Enc");
    client.next().await.unwrap();
    assert_eq!(server.await.unwrap(), offline_uuid("Enc"));
}

#[tokio::test]
async fn an_authenticating_server_needs_an_account() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        let mut c = Connection::new(listener.accept().await.unwrap().0);
        c.read().await.unwrap();
        c.read().await.unwrap();
        c.send(ids::login::clientbound::HELLO, &EncryptionHello { should_authenticate: true, ..Default::default() }).await.unwrap();
        c.read().await.ok();
    });
    assert!(matches!(Client::join_offline("127.0.0.1", port, "Enc").await, Err(Error::NeedsAccount)));
}
