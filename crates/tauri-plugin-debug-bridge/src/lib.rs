use std::{collections::HashMap, sync::Arc};

use axum::{
    Router,
    response::Json,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use tauri::{
    AppHandle, Manager, Runtime,
    plugin::{Builder, TauriPlugin},
};
use tokio::sync::{Mutex, oneshot};
use tower_http::cors::CorsLayer;

mod backend;
mod events;
mod logs;
mod webview;

/// Plugin configuration, read from tauri.conf.json plugin section.
#[derive(Debug, Deserialize, Default)]
pub struct Config {
    /// Port for the debug HTTP/WS server. Defaults to 9229.
    pub port: Option<u16>,
}

/// Pending JS evaluation results, keyed by request ID.
pub type PendingResults = Arc<Mutex<HashMap<String, oneshot::Sender<EvalResult>>>>;

/// Result from a JS evaluation in the webview.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalResult {
    pub success: bool,
    pub value: Option<serde_json::Value>,
    pub error: Option<String>,
}

/// Shared state accessible to all axum route handlers.
pub struct BridgeState<R: Runtime> {
    pub app: AppHandle<R>,
    pub pending: PendingResults,
}

/// Health check response.
#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
    plugin: &'static str,
    version: &'static str,
}

/// Tauri command: receives JS eval results from the webview.
/// Called by injected JS via `window.__TAURI__.invoke('plugin:debug-bridge|eval_callback', ...)`.
#[tauri::command]
async fn eval_callback(
    pending: tauri::State<'_, PendingResults>,
    id: String,
    success: bool,
    value: Option<serde_json::Value>,
    error: Option<String>,
) -> Result<(), String> {
    let mut map = pending.lock().await;
    if let Some(tx) = map.remove(&id) {
        let _ = tx.send(EvalResult {
            success,
            value,
            error,
        });
    }
    Ok(())
}

/// Build the axum router with all debug bridge routes.
fn build_router<R: Runtime>(state: Arc<Mutex<BridgeState<R>>>) -> Router {
    Router::new()
        // Health
        .route("/health", get(health))
        // Webview
        .route("/eval", post(webview::webview_eval::<R>))
        .route("/screenshot", get(webview::screenshot::<R>))
        .route("/snapshot", get(webview::snapshot::<R>))
        .route("/click", post(webview::click::<R>))
        .route("/fill", post(webview::fill::<R>))
        // Backend
        .route("/invoke", post(backend::invoke::<R>))
        .route("/commands", get(backend::commands::<R>))
        .route("/state", get(backend::state::<R>))
        .route("/windows", get(backend::windows::<R>))
        .route("/config", get(backend::config::<R>))
        // Events
        .route("/events/emit", post(events::emit::<R>))
        .route("/events/list", get(events::list::<R>))
        // Logs (WebSocket)
        .route("/logs", get(logs::logs_ws::<R>))
        .route("/console", get(logs::console_ws::<R>))
        .layer(CorsLayer::permissive())
        .with_state(state)
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        plugin: "tauri-plugin-debug-bridge",
        version: env!("CARGO_PKG_VERSION"),
    })
}

/// Initialize the debug bridge plugin.
///
/// ```rust,no_run
/// // In your Tauri app's lib.rs:
/// #[cfg(feature = "debug")]
/// app.plugin(tauri_plugin_debug_bridge::init());
/// ```
pub fn init<R: Runtime>() -> TauriPlugin<R, Option<Config>> {
    let pending: PendingResults = Arc::new(Mutex::new(HashMap::new()));

    Builder::<R, Option<Config>>::new("debug-bridge")
        .invoke_handler(tauri::generate_handler![eval_callback])
        .setup(move |app, api| {
            let port = api.config().as_ref().and_then(|c| c.port).unwrap_or(9229);

            // Share pending results with both the Tauri command and axum handlers.
            app.manage(pending.clone());

            let state = Arc::new(Mutex::new(BridgeState {
                app: app.clone(),
                pending,
            }));

            let router = build_router(state);

            tauri::async_runtime::spawn(async move {
                let addr = format!("127.0.0.1:{port}");
                tracing::info!("debug-bridge listening on http://{addr}");
                let listener = tokio::net::TcpListener::bind(&addr)
                    .await
                    .expect("failed to bind debug-bridge port");
                axum::serve(listener, router)
                    .await
                    .expect("debug-bridge server error");
            });

            Ok(())
        })
        .build()
}
