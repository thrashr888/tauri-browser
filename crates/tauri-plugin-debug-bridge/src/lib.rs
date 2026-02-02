use std::{collections::HashMap, sync::Arc};

use axum::{
    Router,
    extract::DefaultBodyLimit,
    http::{Request, StatusCode},
    middleware::{self, Next},
    response::{Json, Response},
    routing::{get, post},
};
use rand::Rng;
use serde::{Deserialize, Serialize};
use tauri::{
    AppHandle, Manager, Runtime,
    plugin::{Builder, TauriPlugin},
};
use tokio::sync::{Mutex, broadcast, oneshot};

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
    pub console_tx: broadcast::Sender<String>,
}

/// Health check response.
#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
    plugin: &'static str,
    version: &'static str,
}

/// Generate a random 32-character hex token for auth.
fn generate_auth_token() -> String {
    let mut rng = rand::thread_rng();
    let bytes: [u8; 16] = Rng::r#gen(&mut rng);
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Middleware that checks the `X-Debug-Bridge-Token` header on every request
/// except `/health`.
async fn auth_middleware(
    req: Request<axum::body::Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    // Skip auth for health check endpoint.
    if req.uri().path() == "/health" {
        return Ok(next.run(req).await);
    }

    let expected = req
        .extensions()
        .get::<AuthToken>()
        .map(|t| t.0.clone())
        .unwrap_or_default();

    let provided = req
        .headers()
        .get("X-Debug-Bridge-Token")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    if provided != expected {
        return Err(StatusCode::UNAUTHORIZED);
    }

    Ok(next.run(req).await)
}

/// Wrapper to store the auth token in request extensions.
#[derive(Clone)]
struct AuthToken(String);

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

/// Tauri command: receives JS console messages from the webview.
/// Called by the injected console hook via `__TAURI_INTERNALS__.invoke`.
#[tauri::command]
async fn console_callback(
    console_tx: tauri::State<'_, broadcast::Sender<String>>,
    level: String,
    message: String,
) -> Result<(), String> {
    let msg = serde_json::json!({
        "level": level,
        "message": message,
    });
    let _ = console_tx.send(msg.to_string());
    Ok(())
}

/// Build the axum router with all debug bridge routes.
fn build_router<R: Runtime>(state: Arc<BridgeState<R>>, token: String) -> Router {
    let auth_token = AuthToken(token);

    // Stateful routes (require BridgeState via axum State extractor).
    let stateful = Router::new()
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
        .route("/events/listen", get(events::listen::<R>))
        // Logs (WebSocket)
        .route("/logs", get(logs::logs_ws::<R>))
        .route("/console", get(logs::console_ws::<R>))
        .with_state(state);

    // Combine stateless health route with stateful routes, then apply security layers.
    Router::new()
        .route("/health", get(health))
        .merge(stateful)
        // Security: 1 MB body size limit
        .layer(DefaultBodyLimit::max(1_048_576))
        // Security: auth token middleware
        .layer(axum::Extension(auth_token))
        .layer(middleware::from_fn(auth_middleware))
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
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auth_token_format() {
        let token = generate_auth_token();
        assert_eq!(token.len(), 32, "token should be 32 hex chars");
        assert!(
            token.chars().all(|c| c.is_ascii_hexdigit()),
            "token should only contain hex chars"
        );
    }

    #[test]
    fn auth_tokens_are_unique() {
        let t1 = generate_auth_token();
        let t2 = generate_auth_token();
        assert_ne!(t1, t2, "consecutive tokens should differ");
    }
}

pub fn init<R: Runtime>() -> TauriPlugin<R, Option<Config>> {
    let pending: PendingResults = Arc::new(Mutex::new(HashMap::new()));

    Builder::<R, Option<Config>>::new("debug-bridge")
        .invoke_handler(tauri::generate_handler![eval_callback, console_callback])
        .setup(move |app, api| {
            let port = api.config().as_ref().and_then(|c| c.port).unwrap_or(9229);

            // Generate auth token for this session.
            let token = generate_auth_token();
            println!("debug-bridge auth token: {token}");
            tracing::info!("debug-bridge auth token: {token}");

            // Broadcast channel for JS console messages.
            let (console_tx, _) = broadcast::channel(256);

            // Share state with both Tauri commands and axum handlers.
            app.manage(pending.clone());
            app.manage(console_tx.clone());

            let state = Arc::new(BridgeState {
                app: app.clone(),
                pending,
                console_tx,
            });

            let router = build_router(state, token);

            tauri::async_runtime::spawn(async move {
                let addr = format!("127.0.0.1:{port}");
                tracing::info!("debug-bridge listening on http://{addr}");
                let listener = match tokio::net::TcpListener::bind(&addr).await {
                    Ok(l) => l,
                    Err(e) => {
                        tracing::error!("failed to bind debug-bridge on {addr}: {e}");
                        return;
                    }
                };
                if let Err(e) = axum::serve(listener, router).await {
                    tracing::error!("debug-bridge server error: {e}");
                }
            });

            Ok(())
        })
        .build()
}
