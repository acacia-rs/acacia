//! Xbox Live Real-Time Activity (RTA) WebSocket: JSON-array frames (docs/research/friends-join.md §4).

use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio_tungstenite::tungstenite::Message;

use crate::signaling::ws::{self, ws_error};
use crate::socks5::Socks5Proxy;
use crate::ConnectError;

pub(crate) const URL: &str = "wss://rta.xboxlive.com/connect";
const SUBPROTOCOL: &str = "rta.xboxlive.com.V2";
const SUBSCRIBE: u64 = 1;
const UNSUBSCRIBE: u64 = 2;
const EVENT: u64 = 3;
const RESYNC: u64 = 4;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Frame {
    Subscribed { seq: u64, status: u64, sub_id: i64, data: Value },
    Unsubscribed { seq: u64, status: u64 },
    Event { sub_id: i64, data: Value },
    /// The service lost events; re-read whatever the subscriptions cover.
    Resync,
    Unknown,
}

pub(crate) fn subscribe_frame(seq: u64, uri: &str) -> String {
    json!([SUBSCRIBE, seq, uri]).to_string()
}

pub(crate) fn parse(text: &str) -> Frame {
    let Ok(Value::Array(a)) = serde_json::from_str::<Value>(text) else { return Frame::Unknown };
    let u = |i: usize| a.get(i).and_then(Value::as_u64);
    let data = |i: usize| a.get(i).cloned().unwrap_or(Value::Null);
    match (u(0), u(1)) {
        (Some(SUBSCRIBE), Some(seq)) => match (u(2), a.get(3).and_then(Value::as_i64)) {
            (Some(status), Some(sub_id)) => Frame::Subscribed { seq, status, sub_id, data: data(4) },
            (Some(status), None) => Frame::Subscribed { seq, status, sub_id: -1, data: Value::Null },
            _ => Frame::Unknown,
        },
        (Some(UNSUBSCRIBE), Some(seq)) => u(2).map_or(Frame::Unknown, |status| Frame::Unsubscribed { seq, status }),
        (Some(EVENT), _) => a.get(1).and_then(Value::as_i64).map_or(Frame::Unknown, |sub_id| Frame::Event { sub_id, data: data(2) }),
        (Some(RESYNC), _) => Frame::Resync,
        _ => Frame::Unknown,
    }
}

pub(crate) struct Rta {
    socket: ws::Socket,
    next_seq: u64,
}

impl Rta {
    /// `headers` are the signed Xbox Live WebSocket headers for [`URL`].
    pub async fn connect(mut headers: Vec<(&'static str, String)>, proxy: Option<&Socks5Proxy>) -> Result<Self, ConnectError> {
        headers.push(("sec-websocket-protocol", SUBPROTOCOL.to_owned()));
        let (socket, _) = ws::connect(URL, &headers, proxy).await?;
        Ok(Self { socket, next_seq: 1 })
    }

    /// Subscribes to `uri`; returns the subscription id and its initial data. Events that arrive
    /// meanwhile belong to no subscription of ours yet, so they are dropped.
    pub async fn subscribe(&mut self, uri: &str) -> Result<(i64, Value), ConnectError> {
        let seq = self.next_seq;
        self.next_seq += 1;
        self.socket.send(Message::Text(subscribe_frame(seq, uri).into())).await.map_err(ws_error)?;
        let reply = async {
            loop {
                if let Frame::Subscribed { seq: s, status, sub_id, data } = self.next().await? {
                    if s != seq {
                        continue;
                    }
                    return match status {
                        0 => Ok((sub_id, data)),
                        _ => Err(ConnectError::Signaling(format!("RTA subscribe {uri}: status {status}"))),
                    };
                }
            }
        };
        tokio::time::timeout(REQUEST_TIMEOUT, reply).await.map_err(|_| ConnectError::Timeout)?
    }

    /// The next frame; Err once the socket is closed.
    pub async fn next(&mut self) -> Result<Frame, ConnectError> {
        loop {
            match self.socket.next().await {
                Some(Ok(Message::Text(text))) => {
                    tracing::trace!(%text, "rta in");
                    return Ok(parse(text.as_str()));
                }
                Some(Ok(Message::Close(close))) => return Err(ConnectError::Signaling(format!("RTA closed: {close:?}"))),
                Some(Ok(_)) => {}
                Some(Err(e)) => return Err(ws_error(e)),
                None => return Err(ConnectError::Signaling("RTA closed".into())),
            }
        }
    }

    pub async fn close(mut self) {
        let _ = self.socket.close(None).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_round_trip_the_wire_shapes() {
        assert_eq!(subscribe_frame(1, "https://sessiondirectory.xboxlive.com/connections/"), r#"[1,1,"https://sessiondirectory.xboxlive.com/connections/"]"#);
        assert_eq!(
            parse(r#"[1,1,0,42,{"ConnectionId":"a1b2"}]"#),
            Frame::Subscribed { seq: 1, status: 0, sub_id: 42, data: json!({"ConnectionId": "a1b2"}) }
        );
        assert_eq!(parse("[1,2,1001]"), Frame::Subscribed { seq: 2, status: 1001, sub_id: -1, data: Value::Null });
        assert_eq!(parse("[2,3,0]"), Frame::Unsubscribed { seq: 3, status: 0 });
        assert_eq!(
            parse(r#"[3,42,{"shoulderTaps":[{"resource":"x","changeNumber":5}]}]"#),
            Frame::Event { sub_id: 42, data: json!({"shoulderTaps": [{"resource": "x", "changeNumber": 5}]}) }
        );
        assert_eq!(parse("[4]"), Frame::Resync);
        assert_eq!(parse("{}"), Frame::Unknown);
        assert_eq!(parse("[9,1]"), Frame::Unknown);
    }
}
