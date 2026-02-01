use std::sync::Arc;

use axum::{extract::State, http::StatusCode, response::Json};
use serde::{Deserialize, Serialize};
use tauri::{Emitter, Runtime};
use tokio::sync::Mutex;

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

/// POST /events/emit — emit a Tauri event.
pub async fn emit<R: Runtime>(
    State(state): State<Arc<Mutex<BridgeState<R>>>>,
    Json(req): Json<EmitRequest>,
) -> Result<Json<EmitResponse>, (StatusCode, String)> {
    let state = state.lock().await;
    state
        .app
        .emit(&req.event, req.payload)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(EmitResponse { success: true }))
}

/// GET /events/list — list known event names.
pub async fn list<R: Runtime>(
    State(_state): State<Arc<Mutex<BridgeState<R>>>>,
) -> Result<Json<Vec<EventInfo>>, (StatusCode, String)> {
    // TODO: Tauri doesn't expose a public event registry.
    // We could track events emitted/listened through this plugin.
    Err((
        StatusCode::NOT_IMPLEMENTED,
        "event listing not yet implemented".to_string(),
    ))
}
