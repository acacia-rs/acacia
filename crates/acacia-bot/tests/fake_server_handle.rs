//! Tests driving the fake server past the replayed join: send, recv and kick.

use std::time::Duration;

use acacia_bot::client::{Client, TransportKind};
use acacia_bot::forms::FormReply;
use acacia_bot::proto::packets::{ModalFormRequest, ModalFormResponse};
use acacia_bot::{Bot, BotConfig, BotEvent, DisconnectReason};
use acacia_testserver::{FakeServer, Script};

async fn join(name: &str) -> (FakeServer, Bot) {
    let server = FakeServer::start(Script::bds_spawn()).await.unwrap();
    let builder = Client::builder(server.addr().to_string()).offline(name).transport(TransportKind::RakNet);
    let bot = Bot::connect(builder, BotConfig::default()).await.expect("bot joins the fake server");
    server.spawned().await.expect("script reaches PlayerSpawn");
    (server, bot)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn bot_answers_a_form_the_server_sends() {
    let (server, mut bot) = join("FormTester").await;
    let data = r#"{"type":"modal","title":"t","content":"accept?","button1":"yes","button2":"no"}"#;
    server.send(&ModalFormRequest { form_id: 7, data: data.into() });

    let form = bot.wait_form(Duration::from_secs(5)).await.expect("form shown");
    assert_eq!(form.id, 7);
    bot.answer_form(form.id, FormReply::Modal(true)).await.expect("form answered");
    let reply = tokio::select! {
        r = server.recv::<ModalFormResponse>() => r.expect("form response"),
        _ = async { while bot.next().await.is_some() {} } => panic!("bot left before answering"),
    };
    assert_eq!(reply.form_id, 7);
    assert_eq!(reply.data.as_deref().map(str::trim), Some("true"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn bot_sees_the_kick_message() {
    let (server, mut bot) = join("KickTester").await;
    server.kick("bye");
    let reason = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            match bot.next().await {
                Some(BotEvent::Disconnected(reason)) => return reason,
                Some(_) => {}
                None => panic!("bot ended without a disconnect event"),
            }
        }
    })
    .await
    .expect("disconnect within 5 s");
    assert!(matches!(&reason, DisconnectReason::Kicked { message, .. } if message == "bye"), "{reason:?}");
}
