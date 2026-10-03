//! Xbox multiplayer session directory (MPSD) calls for friends' worlds, as XSAPI makes them
//! (go-xsapi `mpsd`, prismarine-xbox-services). Spec: docs/research/friends-join.md §3-4.

use reqwest::Method;
use serde_json::{Value, json};

use super::friend_world::{FriendWorld, MINECRAFT_SCID, SessionRef, parse_activities};
use super::http::Http;
use super::xsapi::XboxLiveAuth;
use crate::{Error, Result};

const BASE: &str = "https://sessiondirectory.xboxlive.com";
const CONTRACT: &str = "107";
pub(crate) const QUERY_URL: &str = "https://sessiondirectory.xboxlive.com/handles/query?include=relatedInfo,customProperties";
/// The RTA resource whose subscription yields the `ConnectionId` MPSD members carry.
pub const RTA_CONNECTIONS_URI: &str = "https://sessiondirectory.xboxlive.com/connections/";

pub(crate) fn activity_query_body(xuid: &str) -> Value {
    json!({"type": "activity", "scid": MINECRAFT_SCID, "owners": {"people": {"moniker": "people", "monikerXuid": xuid}}})
}

/// Adds the caller as an active member whose session changes reach RTA `connection_id`.
pub(crate) fn join_body(xuid: &str, connection_id: &str, subscription_id: &str) -> Value {
    json!({"members": {"me": {
        "constants": {"system": {"xuid": xuid, "initialize": true}},
        "properties": {"system": {
            "active": true,
            "connection": connection_id,
            "subscription": {"id": subscription_id, "changeTypes": ["everything"]},
        }},
    }}})
}

/// Moves the membership to a new RTA connection after a reconnect.
pub(crate) fn connection_body(connection_id: &str) -> Value {
    json!({"members": {"me": {"properties": {"system": {"active": true, "connection": connection_id}}}}})
}

pub(crate) fn leave_body() -> Value {
    json!({"members": {"me": null}})
}

pub(crate) fn activity_body(session: &SessionRef) -> Value {
    json!({"version": 1, "type": "activity", "sessionRef": session.to_json()})
}

fn xuid(auth: &XboxLiveAuth) -> Result<&str> {
    auth.xuid().ok_or_else(|| Error::Protocol { endpoint: QUERY_URL.into(), reason: "XSTS token has no xuid".into() })
}

fn body(v: &Value) -> Vec<u8> {
    v.to_string().into_bytes()
}

/// Worlds hosted by people the account follows.
pub(crate) async fn friend_worlds(http: &Http, auth: &XboxLiveAuth) -> Result<Vec<FriendWorld>> {
    let req = auth.request(http, Method::POST, QUERY_URL, CONTRACT, body(&activity_query_body(xuid(auth)?)));
    Ok(parse_activities(&http.json::<Value>(req, QUERY_URL).await?))
}

/// Joins the session behind an activity handle; returns the session document.
pub(crate) async fn join(http: &Http, auth: &XboxLiveAuth, handle_id: &str, connection_id: &str) -> Result<Value> {
    let url = format!("{BASE}/handles/{handle_id}/session");
    let subscription = uuid::Uuid::new_v4().to_string().to_uppercase();
    let req = auth.request(http, Method::PUT, &url, CONTRACT, body(&join_body(xuid(auth)?, connection_id, &subscription)));
    http.json(req, &url).await
}

pub(crate) async fn session(http: &Http, auth: &XboxLiveAuth, session: &SessionRef) -> Result<Value> {
    let url = session.url();
    http.json(auth.request(http, Method::GET, &url, CONTRACT, Vec::new()), &url).await
}

/// PUTs `change` into the session (a member update or leave); the reply document is not needed.
pub(crate) async fn write(http: &Http, auth: &XboxLiveAuth, session: &SessionRef, change: &Value) -> Result<()> {
    let url = session.url();
    http.send(auth.request(http, Method::PUT, &url, CONTRACT, body(change)), &url).await.map(drop)
}

/// Publishes the caller's activity in `session`, so their friends see where they are.
pub(crate) async fn set_activity(http: &Http, auth: &XboxLiveAuth, session: &SessionRef) -> Result<()> {
    let url = format!("{BASE}/handles");
    http.send(auth.request(http, Method::POST, &url, CONTRACT, body(&activity_body(session))), &url).await.map(drop)
}
