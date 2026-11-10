//! Microsoft sign-in through `FakeSignIn`: the `msaCode` prompt, the online login, and the cache it leaves.

use std::sync::Arc;
use std::time::Duration;

use acacia_host::{FakeSignIn, HostConfig, FAKE_USER_CODE};
use acacia_testserver::{FakeServer, Script};
use serde_json::{json, Value};

use super::common::{code, Driver};

/// A host whose sign-ins take `ms` of a person's time.
fn fake_host(ms: u64) -> Driver {
    Driver::start_with(HostConfig { sign_in: Some(Arc::new(FakeSignIn::new(Duration::from_millis(ms)))), ..HostConfig::default() })
}

fn msa_code(call: u64) -> impl Fn(&Value) -> bool {
    move |r| r["type"] == "msaCode" && r["call"] == call
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_microsoft_bot_prompts_for_a_device_code_then_joins_online() {
    let server = FakeServer::start(Script::bds_spawn()).await.unwrap();
    let mut d = fake_host(300);
    let addr = server.addr();
    let params = json!({ "host": addr.ip().to_string(), "port": addr.port(), "username": "Alex", "transport": "raknet", "auth": "microsoft" });
    d.call(1, "createBot", params.clone());
    let asked = d.until("the msaCode", msa_code(1)).await;
    let record = &d.seen[asked];
    assert_eq!(record["userCode"], FAKE_USER_CODE);
    assert!(record["message"].as_str().unwrap().contains(FAKE_USER_CODE), "{record}");
    assert_eq!(record.get("bot"), None, "the prompt names the call: the bot has no id the caller knows yet");
    let (done, result) = d.done(1).await;
    assert!(result["ok"]["bot"].is_string(), "createBot failed: {result}");
    assert!(asked < done);
    server.spawned().await.expect("the online login reaches PlayerSpawn");
    d.host.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sign_in_caches_without_connecting_and_answers_with_no_bot() {
    let mut d = fake_host(300);
    d.call(1, "signIn", json!({ "username": "Steve" }));
    let asked = d.until("the msaCode", msa_code(1)).await;
    let (done, result) = d.done(1).await;
    assert!(asked < done);
    assert_eq!(result["ok"]["displayName"], "Steve");
    assert_eq!(d.seen[done].get("bot"), None, "signIn names no bot");

    let server = FakeServer::start(Script::bds_spawn()).await.unwrap();
    d.create_with(&server, 2, "Steve", json!({ "auth": "microsoft" })).await;
    assert!(!d.seen.iter().any(msa_code(2)), "the cached sign-in needs no person");
    d.call(3, "signIn", json!({ "username": "Steve", "profilesFolder": "/nonexistent/other" }));
    d.until("a prompt for another cache", msa_code(3)).await;
    d.done(3).await;
    d.host.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancel_call_abandons_a_sign_in_and_a_signing_in_create_bot() {
    let mut d = fake_host(60_000);
    d.call(1, "signIn", json!({ "username": "Slow", "profilesFolder": "/nonexistent/a" }));
    d.until("the msaCode", msa_code(1)).await;
    d.ok(2, "cancelCall", json!({ "call": 1 })).await;
    assert_eq!(code(&d.done(1).await.1), "CANCELLED");

    let params = json!({ "host": "127.0.0.1", "port": 1, "username": "Slower", "auth": "microsoft", "profilesFolder": "/nonexistent/b" });
    d.call(3, "createBot", params);
    d.until("the msaCode", msa_code(3)).await;
    d.ok(4, "cancelCall", json!({ "call": 3 })).await;
    assert_eq!(code(&d.done(3).await.1), "CANCELLED");
    d.ok(5, "cancelCall", json!({ "call": 3 })).await;
    d.host.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shutdown_answers_a_sign_in_still_waiting_on_a_person() {
    let mut d = fake_host(60_000);
    d.call(1, "signIn", json!({ "username": "Late", "profilesFolder": "/nonexistent/fake" }));
    d.until("the msaCode", msa_code(1)).await;
    d.host.shutdown().await;
    assert_eq!(code(&d.done(1).await.1), "CLOSED");
}
