//! Microsoft sign-in over the socket, with acacia-host's `FakeSignIn`: prompts reach the calling connection only.

use std::sync::Arc;
use std::time::Duration;

use acacia_host::{FakeSignIn, HostConfig, FAKE_USER_CODE};
use acacia_testserver::{FakeServer, Script};
use serde_json::json;

use super::common::{daemon_on, Client};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn msa_codes_go_to_the_calling_connection_under_its_request_id() {
    let server = FakeServer::start(Script::bds_spawn()).await.unwrap();
    let host = HostConfig { sign_in: Some(Arc::new(FakeSignIn::new(Duration::from_millis(300)))), ..HostConfig::default() };
    let d = daemon_on(host, None).await;
    let mut c = Client::greeted(d.local_addr(), json!({})).await;
    let mut other = Client::greeted(d.local_addr(), json!({})).await;
    let addr = server.addr();
    let params = json!({ "host": addr.ip().to_string(), "port": addr.port(), "username": "Online", "transport": "raknet", "auth": "microsoft" });
    let id = c.request("createBot", params).await;
    let asked = c.until("the msaCode", |r| r["type"] == "msaCode").await;
    assert_eq!(c.seen[asked]["call"], id, "renamed to the client's request id");
    assert_eq!(c.seen[asked]["userCode"], FAKE_USER_CODE);
    let (answered, frame) = c.response(id).await;
    assert!(frame["result"]["bot"].is_string(), "{frame}");
    assert!(asked < answered);
    server.spawned().await.expect("the online login reaches PlayerSpawn");

    let signed = other.ok("signIn", json!({ "username": "Online" })).await;
    assert_eq!(signed["displayName"], "Online", "signed in already: no prompt, answered at once");
    assert!(!other.seen.iter().any(|r| r["type"] == "msaCode"));
    assert!(c.seen.iter().filter(|r| r["type"] == "msaCode").count() == 1, "one prompt, to its own connection");

    let refused = other.err("signIn", json!({ "username": "Online", "profilesFolder": "/tmp/elsewhere" })).await;
    assert_eq!(refused, "BAD_PARAMS", "clients cannot pick where tokens are written");
    d.shutdown().await;
}
