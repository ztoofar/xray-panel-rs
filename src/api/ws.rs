//! 3x-ui compatible WebSocket endpoint. The browser client connects to
//! `<basePath>ws` and expects `{type, payload, time}` frames; the traffic job
//! broadcasts the same delta snapshots 3x-ui does. Authentication is the
//! normal session-cookie middleware on the protected router.

use axum::{
    extract::{
        ws::{Message, WebSocket},
        State, WebSocketUpgrade,
    },
    response::IntoResponse,
};
use tokio::sync::broadcast::error::RecvError;

use crate::state::AppState;

pub async fn handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| run(socket, state))
}

async fn run(mut socket: WebSocket, state: AppState) {
    let mut events = state.ws_events.subscribe();
    loop {
        tokio::select! {
            event = events.recv() => match event {
                Ok(text) => {
                    if socket.send(Message::Text(text.into())).await.is_err() {
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
                // The browser sends no application messages; ping/pong is
                // handled by the WebSocket layer.
                Some(Ok(_)) => {}
            }
        }
    }
}
