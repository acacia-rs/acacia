//! MPSD request building and response parsing against synthetic documents shaped like the references'
//! (docs/research/friends-join.md).

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use p256::ecdsa::signature::Verifier;
use p256::ecdsa::{Signature, SigningKey};
use reqwest::Method;
use serde_json::{Value, json};

use super::config::AuthConfig;
use super::friend_world::*;
use super::http::Http;
use super::mpsd;
use super::sign;
use super::xbox::XboxToken;
use super::xsapi::XboxLiveAuth;

const HOST_XUID: &str = "2535400000000001";
const OWN_XUID: &str = "2535400000000002";

fn handle(custom: Value) -> Value {
    json!({
        "id": "6f3c1a52-0f5e-4c9b-9d0a-111111111111",
        "type": "activity",
        "version": 1,
        "sessionRef": {"scid": MINECRAFT_SCID, "templateName": MINECRAFT_TEMPLATE, "name": "A1B2C3D4-0000-4000-8000-000000000000"},
        "titleId": "896928775",
        "ownerXuid": HOST_XUID,
        "relatedInfo": {"closed": false, "joinRestriction": "followed", "maxMembersCount": 8, "membersCount": 1, "visibility": "open"},
        "customProperties": custom,
    })
}

fn host_custom() -> Value {
    json!({
        "hostName": "HostTag", "ownerId": HOST_XUID, "worldName": "My World", "version": "1.26.52",
        "protocol": 2193, "MemberCount": 1, "MaxMemberCount": 8, "Joinability": "joinable_by_friends",
        "BroadcastSetting": 3, "TransportLayer": 2, "LanGame": true, "TitleId": 0, "rakNetGUID": "",
        "SupportedConnections": [
            {"ConnectionType": 7, "HostIpAddress": "", "HostPort": 0, "NetherNetId": 17_000_000_000_000_000_001u64,
             "PmsgId": "0d8a9f3e-1c2b-4a5d-8e7f-222222222222"},
            {"ConnectionType": 3, "HostIpAddress": "", "HostPort": 0, "NetherNetId": "12345"},
        ],
    })
}

#[test]
fn parses_activity_handles() {
    let other_template = json!({"id": "x", "sessionRef": {"scid": MINECRAFT_SCID, "templateName": "Party", "name": "p"}, "customProperties": {}});
    let worlds = parse_activities(&json!({"results": [handle(host_custom()), other_template, {"id": "broken"}]}));
    assert_eq!(worlds.len(), 1);
    let w = &worlds[0];
    assert_eq!((w.owner_xuid.as_str(), w.host_name.as_str(), w.world_name.as_str()), (HOST_XUID, "HostTag", "My World"));
    assert_eq!((w.protocol, w.members, w.max_members, w.closed), (2193, 1, 8, false));
    assert_eq!(w.session.url(), format!(
        "https://sessiondirectory.xboxlive.com/serviceconfigs/{MINECRAFT_SCID}/sessionTemplates/MinecraftLobby/sessions/A1B2C3D4-0000-4000-8000-000000000000"
    ));
    let rpc = &w.connections[0];
    assert_eq!(rpc.kind, ConnectionKind::SignalingJsonRpc);
    assert_eq!(rpc.nethernet_id.as_deref(), Some("17000000000000000001"), "u64 ids survive as numbers");
    assert_eq!(rpc.pmsg_id.as_deref(), Some("0d8a9f3e-1c2b-4a5d-8e7f-222222222222"));
    assert_eq!((rpc.host_ip.as_deref(), rpc.host_port), (None, 0));
    assert_eq!((w.connections[1].kind, w.connections[1].nethernet_id.as_deref()), (ConnectionKind::SignalingLegacy, Some("12345")));
}

#[test]
fn reads_the_published_nonce() {
    let mut doc = json!({"properties": {"custom": host_custom()}});
    assert_eq!(session_nonce(&doc, OWN_XUID), None);
    doc["properties"]["custom"]["nonces"] = json!({OWN_XUID: "", "1": "other"});
    assert_eq!(session_nonce(&doc, OWN_XUID), None, "an empty nonce is not published yet");
    doc["properties"]["custom"]["nonces"][OWN_XUID] = json!("9f86d081884c7d65");
    assert_eq!(session_nonce(&doc, OWN_XUID).as_deref(), Some("9f86d081884c7d65"));
}

#[test]
fn builds_request_bodies() {
    assert_eq!(
        mpsd::activity_query_body(OWN_XUID),
        json!({"type": "activity", "scid": MINECRAFT_SCID, "owners": {"people": {"moniker": "people", "monikerXuid": OWN_XUID}}})
    );
    let join = mpsd::join_body(OWN_XUID, "conn-1", "SUB-1");
    assert_eq!(join["members"]["me"]["constants"]["system"], json!({"xuid": OWN_XUID, "initialize": true}));
    assert_eq!(
        join["members"]["me"]["properties"]["system"],
        json!({"active": true, "connection": "conn-1", "subscription": {"id": "SUB-1", "changeTypes": ["everything"]}})
    );
    assert_eq!(mpsd::connection_body("conn-2"), json!({"members": {"me": {"properties": {"system": {"active": true, "connection": "conn-2"}}}}}));
    assert_eq!(mpsd::leave_body(), json!({"members": {"me": null}}));
    let w = &parse_activities(&json!({"results": [handle(host_custom())]}))[0];
    assert_eq!(mpsd::activity_body(&w.session)["sessionRef"]["templateName"], MINECRAFT_TEMPLATE);
}

fn auth() -> XboxLiveAuth {
    let token = XboxToken { token: "tok".into(), not_after: i64::MAX, user_hash: Some("uhs".into()), xuid: Some(OWN_XUID.into()), gamertag: None };
    XboxLiveAuth { token, key: SigningKey::from_slice(&[7; 32]).unwrap() }
}

#[test]
fn signs_requests_like_xsapi() {
    let http = Http::new(&AuthConfig::default()).unwrap();
    let auth = auth();
    let body = mpsd::activity_query_body(OWN_XUID).to_string().into_bytes();
    let req = auth.request(&http, Method::POST, mpsd::QUERY_URL, "107", body.clone()).build().unwrap();
    let names: Vec<&str> = req.headers().keys().map(|k| k.as_str()).collect();
    assert_eq!(names, ["x-xbl-contract-version", "content-type", "accept-language", "user-agent", "authorization", "signature"]);
    let h = |n: &str| req.headers()[n].to_str().unwrap().to_owned();
    assert_eq!(h("authorization"), "XBL3.0 x=uhs;tok");
    assert!(h("user-agent").starts_with("XboxServicesAPI/") && h("user-agent").ends_with(" c"));

    let raw = STANDARD.decode(h("signature")).unwrap();
    let filetime = i64::from_be_bytes(raw[4..12].try_into().unwrap());
    let input = sign::signing_input(filetime, "POST", "/handles/query?include=relatedInfo,customProperties", "XBL3.0 x=uhs;tok", &body);
    auth.key.verifying_key().verify(&input, &Signature::from_slice(&raw[12..]).unwrap()).expect("signature covers path, query and body");
}

#[test]
fn websocket_headers_sign_the_connect_path() {
    let http = Http::new(&AuthConfig::default()).unwrap();
    let auth = auth();
    let headers = auth.websocket_headers(&http, "wss://rta.xboxlive.com/connect");
    let get = |n: &str| headers.iter().find(|(k, _)| *k == n).map(|(_, v)| v.clone()).unwrap();
    assert!(!get("user-agent").ends_with(" c"));
    let raw = STANDARD.decode(get("signature")).unwrap();
    let filetime = i64::from_be_bytes(raw[4..12].try_into().unwrap());
    let input = sign::signing_input(filetime, "GET", "/connect", "XBL3.0 x=uhs;tok", b"");
    auth.key.verifying_key().verify(&input, &Signature::from_slice(&raw[12..]).unwrap()).unwrap();
}
