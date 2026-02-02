use std::sync::Arc;

use axum::{
    extract::{
        Query, State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    http::StatusCode,
    response::{Json, Response},
};
use serde::{Deserialize, Serialize};
use tauri::{Emitter, Listener, Runtime};
use tokio::sync::mpsc;

use crate::BridgeState;

#[derive(Deserialize)]
pub struct EmitRequest {
    pub event: String,
    #[serde(default)]
    pub payload: serde_json::Value,
}

#[derive(Serialize)]
pub struct EmitResponse {
    pub success: bool,
}

#[derive(Serialize)]
pub struct EventInfo {
    pub name: String,
}

#[derive(Deserialize)]
pub struct ListenQuery {
    pub name: String,
}

/// POST /events/emit — emit a Tauri event.
pub async fn emit<R: Runtime>(
    State(state): State<Arc<BridgeState<R>>>,
    Json(req): Json<EmitRequest>,
) -> Result<Json<EmitResponse>, (StatusCode, String)> {
    state
        .app
        .emit(&req.event, req.payload)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(EmitResponse { success: true }))
}

/// GET /events/list — list known event names.
pub async fn list<R: Runtime>(
    State(_state): State<Arc<BridgeState<R>>>,
) -> Result<Json<Vec<EventInfo>>, (StatusCode, String)> {
    // Tauri doesn't expose a public event registry.
    Err((
        StatusCode::NOT_IMPLEMENTED,
        "event listing not yet implemented — Tauri has no public event registry".to_string(),
    ))
}

/// GET /events/listen?name=<event> — WebSocket stream of Tauri events.
pub async fn listen<R: Runtime>(
    State(state): State<Arc<BridgeState<R>>>,
    Query(query): Query<ListenQuery>,
    ws: WebSocketUpgrade,
) -> Response {
    let app = state.app.clone();
    let event_name = query.name;
    ws.on_upgrade(move |socket| handle_listen(socket, app, event_name))
}

async fn handle_listen<R: Runtime>(
    mut socket: WebSocket,
    app: tauri::AppHandle<R>,
    event_name: String,
) {
    let (tx, mut rx) = mpsc::channel::<String>(64);

    // Subscribe to the Tauri event.
    let name_for_closure = event_name.clone();
    let event_id = app.listen(&event_name, move |event| {
        let msg = serde_json::json!({
            "event": name_for_closure,
            "payload": event.payload(),
        });
        let _ = tx.try_send(msg.to_string());
    });

    // Forward events to the WebSocket client until disconnect.
    loop {
        tokio::select! {
            Some(msg) = rx.recv() => {
                if socket.send(Message::Text(msg.into())).await.is_err() {
                    break;
                }
            }
            Some(Ok(msg)) = socket.recv() => {
                if matches!(msg, Message::Close(_)) {
                    break;
                }
            }
            else => break,
        }
    }

    // Clean up the Tauri event listener.
    app.unlisten(event_id);
}
