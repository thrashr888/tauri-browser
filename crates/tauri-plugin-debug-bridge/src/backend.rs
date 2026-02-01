use std::sync::Arc;

use axum::{extract::State, http::StatusCode, response::Json};
use serde::{Deserialize, Serialize};
use tauri::{Manager, Runtime};
use tokio::sync::Mutex;

use crate::{BridgeState, EvalResult};

#[derive(Deserialize)]
pub struct InvokeRequest {
    pub command: String,
    #[serde(default)]
    pub args: serde_json::Value,
}

#[derive(Serialize)]
pub struct CommandInfo {
    pub name: String,
}

#[derive(Serialize)]
pub struct WindowInfo {
    pub label: String,
    pub title: Option<String>,
    pub url: Option<String>,
    pub is_visible: bool,
    pub is_focused: bool,
}

/// POST /invoke — call a registered Tauri command by routing through the webview.
/// Since Tauri doesn't expose a Rust-side command invocation API, we inject JS
/// that calls `window.__TAURI__.core.invoke()` and captures the result.
pub async fn invoke<R: Runtime>(
    State(state): State<Arc<Mutex<BridgeState<R>>>>,
    Json(req): Json<InvokeRequest>,
) -> Result<Json<EvalResult>, (StatusCode, String)> {
    let state = state.lock().await;
    let window = state
        .app
        .get_webview_window("main")
        .ok_or_else(|| (StatusCode::NOT_FOUND, "window 'main' not found".to_string()))?;

    let args_json = serde_json::to_string(&req.args)
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("invalid args: {e}")))?;

    let invoke_js = format!(
        r#"
        try {{
            const result = await window.__TAURI__.core.invoke({cmd}, {args});
            return result;
        }} catch(e) {{
            throw new Error('invoke failed: ' + e);
        }}
        "#,
        cmd = serde_json::to_string(&req.command).unwrap(),
        args = args_json,
    );

    let id = crate::webview::uuid_v4();
    let (tx, rx) = tokio::sync::oneshot::channel();

    {
        let mut pending = state.pending.lock().await;
        pending.insert(id.clone(), tx);
    }

    // Wrap the invoke JS with the callback mechanism to return the result
    // through the plugin's IPC channel (Tauri WebviewWindow API, dev-only).
    let wrapped = format!(
        r#"(async () => {{
            try {{
                const __result = await (async () => {{ {code} }})();
                await window.__TAURI__.core.invoke(
                    'plugin:debug-bridge|eval_callback',
                    {{ id: '{id}', success: true, value: __result, error: null }}
                );
            }} catch(__e) {{
                await window.__TAURI__.core.invoke(
                    'plugin:debug-bridge|eval_callback',
                    {{ id: '{id}', success: false, value: null, error: __e.toString() }}
                );
            }}
        }})()"#,
        code = invoke_js,
        id = id,
    );

    window
        .eval(&wrapped)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    match tokio::time::timeout(std::time::Duration::from_secs(30), rx).await {
        Ok(Ok(result)) => Ok(Json(result)),
        Ok(Err(_)) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            "invoke callback channel dropped".to_string(),
        )),
        Err(_) => {
            let mut pending = state.pending.lock().await;
            pending.remove(&id);
            Err((
                StatusCode::GATEWAY_TIMEOUT,
                "invoke timed out after 30s".to_string(),
            ))
        }
    }
}

/// GET /commands — list all registered Tauri commands.
/// Since Tauri doesn't expose a public command registry, this endpoint
/// is a placeholder that apps can populate via the plugin API.
pub async fn commands<R: Runtime>(
    State(_state): State<Arc<Mutex<BridgeState<R>>>>,
) -> Result<Json<Vec<CommandInfo>>, (StatusCode, String)> {
    // TODO: Allow apps to register command metadata with the plugin.
    Ok(Json(vec![]))
}

/// GET /state — dump managed state.
/// Placeholder — apps need to register serializable state with the plugin.
pub async fn state<R: Runtime>(
    State(_state): State<Arc<Mutex<BridgeState<R>>>>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    Ok(Json(serde_json::json!({
        "note": "state inspection requires app integration — register state types with the plugin"
    })))
}

/// GET /windows — list all open windows/webviews.
pub async fn windows<R: Runtime>(
    State(state): State<Arc<Mutex<BridgeState<R>>>>,
) -> Result<Json<Vec<WindowInfo>>, (StatusCode, String)> {
    let state = state.lock().await;
    let webview_windows = state.app.webview_windows();

    let windows: Vec<WindowInfo> = webview_windows
        .iter()
        .map(|(label, w)| WindowInfo {
            label: label.clone(),
            title: w.title().ok(),
            url: w.url().ok().map(|u| u.to_string()),
            is_visible: w.is_visible().unwrap_or(false),
            is_focused: w.is_focused().unwrap_or(false),
        })
        .collect();

    Ok(Json(windows))
}

/// GET /config — return the app's Tauri config.
pub async fn config<R: Runtime>(
    State(state): State<Arc<Mutex<BridgeState<R>>>>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let state = state.lock().await;
    let config = state.app.config();
    let json = serde_json::to_value(config)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(json))
}
