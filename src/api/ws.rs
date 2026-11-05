//! 3x-ui compatible WebSocket endpoint. The browser client connects to
//! `<basePath>ws` and expects `{type, payload, time}` frames; the traffic job
//! broadcasts the same delta snapshots 3x-ui does. Authentication is the
//! normal session-cookie middleware on the protected router.

use std::time::Duration;

use axum::{
    extract::{
        ws::{Message, WebSocket},
        State, WebSocketUpgrade,
    },
    response::IntoResponse,
};
use tokio::{
    sync::broadcast::error::RecvError,
    time::{interval, Instant},
};

use crate::state::AppState;

// Browsers answer our pings with pongs; a peer that stays silent for this long
// is gone (laptop sleep, dropped mobile network, half-open NAT mapping).
const PING_INTERVAL: Duration = Duration::from_secs(30);
const IDLE_TIMEOUT: Duration = Duration::from_secs(90);
// A stalled peer must not pin a task and the queued frame forever.
const SEND_TIMEOUT: Duration = Duration::from_secs(10);

pub async fn handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| run(socket, state))
}

async fn run(mut socket: WebSocket, state: AppState) {
    let mut events = state.ws_events.subscribe();
    let mut ping = interval(PING_INTERVAL);
    let mut last_seen = Instant::now();

    loop {
        tokio::select! {
            _ = ping.tick() => {
                if last_seen.elapsed() >= IDLE_TIMEOUT {
                    break;
                }
                let sent = tokio::time::timeout(SEND_TIMEOUT, socket.send(Message::Ping(Vec::new().into())))
                    .await;
                if !matches!(sent, Ok(Ok(()))) {
                    break;
                }
            }
            event = events.recv() => match event {
                Ok(text) => {
                    let sent = tokio::time::timeout(SEND_TIMEOUT, socket.send(Message::Text(text.into())))
                        .await;
                    if !matches!(sent, Ok(Ok(()))) {
                        break;
                    }
                }
                // A slow client missed frames; the next one carries the newest
                // rates and online sets, so skipping is harmless.
                Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => break,
            },
            incoming = socket.recv() => match incoming {
                None | Some(Err(_)) | Some(Ok(Message::Close(_))) => break,
                // Browsers send no application messages, but they do answer
                // our pings; any inbound frame proves the peer is still there.
                Some(Ok(_)) => last_seen = Instant::now(),
            }
        }
    }
}
