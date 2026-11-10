//! Accepting WebSocket connections and moving their frames to and from the router.

use futures_util::{SinkExt, StreamExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::handshake::server::{ErrorResponse, Request, Response};
use tokio_tungstenite::tungstenite::http::StatusCode;
use tokio_tungstenite::tungstenite::Message;

use crate::router::{ConnId, ToRouter};

pub(crate) async fn accept(listener: TcpListener, router: mpsc::UnboundedSender<ToRouter>, queue: usize, token: bool) {
    let mut next: ConnId = 0;
    loop {
        let Ok((tcp, _)) = listener.accept().await else { continue };
        next += 1;
        tokio::spawn(serve(tcp, next, router.clone(), queue, token));
    }
}

/// Without a token, a page in a browser could drive the daemon's bots; browsers always send `Origin`, Node does not.
#[allow(clippy::result_large_err)] // tungstenite's callback signature
fn check_origin(token: bool) -> impl FnOnce(&Request, Response) -> Result<Response, ErrorResponse> {
    move |request, response| {
        if token || !request.headers().contains_key("origin") {
            return Ok(response);
        }
        let mut refused = ErrorResponse::new(Some("browser origins need the daemon's --token".into()));
        *refused.status_mut() = StatusCode::FORBIDDEN;
        Err(refused)
    }
}

async fn serve(tcp: TcpStream, conn: ConnId, router: mpsc::UnboundedSender<ToRouter>, queue: usize, token: bool) {
    let _ = tcp.set_nodelay(true);
    let Ok(ws) = tokio_tungstenite::accept_hdr_async(tcp, check_origin(token)).await else { return };
    let (mut sink, mut stream) = ws.split();
    let (tx, mut rx) = mpsc::channel::<String>(queue);
    if router.send(ToRouter::Open { conn, tx }).is_err() {
        return;
    }
    let writer = async move {
        while let Some(frame) = rx.recv().await {
            if sink.send(Message::text(frame)).await.is_err() {
                break;
            }
        }
        let _ = sink.close().await;
    };
    let reader = async {
        while let Some(Ok(message)) = stream.next().await {
            let text = match message {
                Message::Text(text) => text.as_str().to_owned(),
                Message::Binary(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
                Message::Close(_) => break,
                _ => continue,
            };
            if router.send(ToRouter::Text { conn, text }).is_err() {
                break;
            }
        }
        let _ = router.send(ToRouter::Closed { conn });
    };
    tokio::join!(writer, reader);
}
