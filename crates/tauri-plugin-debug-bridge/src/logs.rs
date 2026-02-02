use std::sync::Arc;

use axum::{
    extract::{
        State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    response::Response,
};
use tauri::Runtime;

use crate::BridgeState;

/// GET /logs — WebSocket endpoint for streaming Rust-side logs.
pub async fn logs_ws<R: Runtime>(
    State(_state): State<Arc<BridgeState<R>>>,
    ws: WebSocketUpgrade,
) -> Response {
    ws.on_upgrade(handle_logs)
}

async fn handle_logs(mut socket: WebSocket) {
    // TODO: Hook into tracing subscriber and forward log events.
    // For now, send a placeholder and keep connection open.
    let _ = socket
        .send(Message::Text(
            r#"{"type":"info","message":"log streaming not yet implemented"}"#.into(),
        ))
        .await;

    // Keep the connection alive until the client disconnects.
    while let Some(Ok(_msg)) = socket.recv().await {
        // Client messages (e.g., filter changes) will be handled here.
    }
}

/// GET /console — WebSocket endpoint for streaming JS console output.
pub async fn console_ws<R: Runtime>(
    State(_state): State<Arc<BridgeState<R>>>,
    ws: WebSocketUpgrade,
) -> Response {
    ws.on_upgrade(handle_console)
}

async fn handle_console(mut socket: WebSocket) {
    // TODO: Inject console-capturing JS into the webview and forward output.
    let _ = socket
        .send(Message::Text(
            r#"{"type":"info","message":"console streaming not yet implemented"}"#.into(),
        ))
        .await;

    while let Some(Ok(_msg)) = socket.recv().await {}
}
