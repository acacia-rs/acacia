//! A stand-in server walks the client through login queries, configuration's answers and a reconfiguration in play.

use acacia_java_proto::packets::{
    Brand, ClientInformation, CookieRequest, CookieResponse, CustomQueryAnswer, Empty, KeepAlive, LoginFinished, Ping, ResourcePackPush,
    ResourcePackResponse, StoreCookie,
};
use acacia_java_proto::{VarInt, Wire, ids};
use tokio::net::{TcpListener, TcpStream};

use crate::frame::Connection;
use crate::join::Client;

async fn expect<T: Wire>(c: &mut Connection, id: i32) -> T {
    let (got, body) = c.read().await.unwrap();
    assert_eq!(got, id, "packet id");
    T::read(&mut &body[..]).unwrap()
}

async fn serve(stream: TcpStream) {
    use ids::{configuration as cfg, login, play};
    let mut c = Connection::new(stream);
    c.read().await.unwrap();
    c.read().await.unwrap();

    c.send(login::clientbound::COOKIE_REQUEST, &CookieRequest { key: "t:k".into() }).await.unwrap();
    let cookie: CookieResponse = expect(&mut c, login::serverbound::COOKIE_RESPONSE).await;
    assert_eq!(cookie, CookieResponse { key: "t:k".into(), payload: None });
    let mut query = Vec::new();
    VarInt(7).write(&mut query);
    "velocity:player_info".to_owned().write(&mut query);
    c.write(crate::frame::encode(login::clientbound::CUSTOM_QUERY, &Raw(query))).await.unwrap();
    let answer: CustomQueryAnswer = expect(&mut c, login::serverbound::CUSTOM_QUERY_ANSWER).await;
    assert_eq!(answer, CustomQueryAnswer { transaction: VarInt(7), payload: None });

    c.send(login::clientbound::LOGIN_FINISHED, &LoginFinished { uuid: 1, name: "Cfg".into() }).await.unwrap();
    expect::<Empty>(&mut c, login::serverbound::LOGIN_ACKNOWLEDGED).await;
    let brand: Brand = expect(&mut c, cfg::serverbound::CUSTOM_PAYLOAD).await;
    assert_eq!(brand.brand, "vanilla");
    let info: ClientInformation = expect(&mut c, cfg::serverbound::CLIENT_INFORMATION).await;
    assert_eq!(info.language, "en_us");

    c.send(cfg::clientbound::STORE_COOKIE, &StoreCookie { key: "t:k".into(), payload: vec![1, 2] }).await.unwrap();
    c.send(cfg::clientbound::COOKIE_REQUEST, &CookieRequest { key: "t:k".into() }).await.unwrap();
    let cookie: CookieResponse = expect(&mut c, cfg::serverbound::COOKIE_RESPONSE).await;
    assert_eq!(cookie.payload, Some(vec![1, 2]));
    c.send(cfg::clientbound::RESOURCE_PACK_PUSH, &ResourcePackPush { id: 9, url: "http://x/p.zip".into(), hash: String::new(), required: true }).await.unwrap();
    for action in [3, 4, 0] {
        let r: ResourcePackResponse = expect(&mut c, cfg::serverbound::RESOURCE_PACK).await;
        assert_eq!(r, ResourcePackResponse { id: 9, action: VarInt(action) });
    }
    c.send(cfg::clientbound::CODE_OF_CONDUCT, &"Be kind".to_owned()).await.unwrap();
    expect::<Empty>(&mut c, cfg::serverbound::ACCEPT_CODE_OF_CONDUCT).await;
    c.send(cfg::clientbound::FINISH_CONFIGURATION, &Empty {}).await.unwrap();
    expect::<Empty>(&mut c, cfg::serverbound::FINISH_CONFIGURATION).await;

    // A proxy switching backends: back to configuration, then play again.
    c.send(play::clientbound::START_CONFIGURATION, &Empty {}).await.unwrap();
    expect::<Empty>(&mut c, play::serverbound::CONFIGURATION_ACKNOWLEDGED).await;
    c.send(cfg::clientbound::KEEP_ALIVE, &KeepAlive { id: 5 }).await.unwrap();
    assert_eq!(expect::<KeepAlive>(&mut c, cfg::serverbound::KEEP_ALIVE).await.id, 5, "the client's play packet must not leak in");
    c.send(cfg::clientbound::FINISH_CONFIGURATION, &Empty {}).await.unwrap();
    expect::<Empty>(&mut c, cfg::serverbound::FINISH_CONFIGURATION).await;
    c.send(play::clientbound::PING, &Ping { id: 9 }).await.unwrap();
    assert_eq!(expect::<Ping>(&mut c, play::serverbound::PONG).await.id, 9);
}

/// A body written as is.
struct Raw(Vec<u8>);

impl Wire for Raw {
    fn read(input: &mut &[u8]) -> Result<Self, acacia_java_proto::Error> {
        Ok(Raw(std::mem::take(input).to_vec()))
    }
    fn write(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.0);
    }
}

#[tokio::test]
async fn answers_configuration_and_resumes_play_after_a_reconfiguration() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = tokio::spawn(async move { serve(listener.accept().await.unwrap().0).await });
    let mut client = Client::join_offline("127.0.0.1", port, "Cfg").await.unwrap();
    assert_eq!(client.next().await.unwrap().id, ids::play::clientbound::START_CONFIGURATION);
    assert!(client.configuring());
    client.send(ids::play::serverbound::PONG, &Ping { id: 1 }).unwrap();
    assert_eq!(client.next().await.unwrap().id, ids::play::clientbound::PING);
    assert!(!client.configuring());
    server.await.unwrap();
}
