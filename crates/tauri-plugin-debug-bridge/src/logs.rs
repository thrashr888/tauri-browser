use std::sync::Arc;

use axum::{
    extract::{
        State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    response::Response,
};
use tauri::{Manager, Runtime};

use crate::BridgeState;

/// JavaScript that hooks console.log/warn/error/info and forwards messages
/// to the debug bridge plugin via `__TAURI_INTERNALS__.invoke`.
/// Idempotent — checks a flag to avoid double-hooking.
const CONSOLE_HOOK_JS: &str = r#"
(function() {
    if (window.__debugBridgeConsoleHooked) return;
    window.__debugBridgeConsoleHooked = true;

    function hook(level, origFn) {
        return function(...args) {
            origFn.apply(console, args);
            try {
                const parts = args.map(a => {
                    try { return typeof a === 'string' ? a : JSON.stringify(a); }
                    catch { return String(a); }
                });
                window.__TAURI_INTERNALS__.invoke(
                    'plugin:debug-bridge|console_callback',
                    { level: level, message: parts.join(' ') }
                );
            } catch(e) {}
        };
    }

    console.log = hook('log', console.log.bind(console));
    console.warn = hook('warn', console.warn.bind(console));
    console.error = hook('error', console.error.bind(console));
    console.info = hook('info', console.info.bind(console));
    console.debug = hook('debug', console.debug.bind(console));

    // Also capture unhandled errors and promise rejections.
    window.addEventListener('error', function(e) {
        window.__TAURI_INTERNALS__.invoke(
            'plugin:debug-bridge|console_callback',
            { level: 'error', message: e.message + ' at ' + e.filename + ':' + e.lineno }
        );
    });
    window.addEventListener('unhandledrejection', function(e) {
        window.__TAURI_INTERNALS__.invoke(
            'plugin:debug-bridge|console_callback',
            { level: 'error', message: 'Unhandled rejection: ' + String(e.reason) }
        );
    });
})();
"#;

/// GET /logs — WebSocket endpoint for streaming Rust-side logs.
pub async fn logs_ws<R: Runtime>(
    State(_state): State<Arc<BridgeState<R>>>,
    ws: WebSocketUpgrade,
) -> Response {
    ws.on_upgrade(handle_logs)
}

async fn handle_logs(mut socket: WebSocket) {
    // Log streaming requires the host app to add a tracing layer.
    // Send a diagnostic message and keep the connection open.
    let _ = socket
        .send(Message::Text(
            serde_json::json!({
                "level": "info",
                "message": "log streaming connected — host app tracing integration required for live logs"
            })
            .to_string()
            .into(),
        ))
        .await;

    // Keep alive until client disconnects.
    while let Some(Ok(msg)) = socket.recv().await {
        if matches!(msg, Message::Close(_)) {
            break;
        }
    }
}

/// GET /console — WebSocket endpoint for streaming JS console output.
/// Injects a console hook into the webview on first connection, then
/// streams all console.log/warn/error/info messages to the client.
pub async fn console_ws<R: Runtime>(
    State(state): State<Arc<BridgeState<R>>>,
    ws: WebSocketUpgrade,
) -> Response {
    let app = state.app.clone();
    let console_tx = state.console_tx.clone();
    ws.on_upgrade(move |socket| handle_console(socket, app, console_tx))
}

async fn handle_console<R: Runtime>(
    mut socket: WebSocket,
    app: tauri::AppHandle<R>,
    console_tx: tokio::sync::broadcast::Sender<String>,
) {
    // Inject the console hook into the main webview.
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.eval(CONSOLE_HOOK_JS);
    }

    // Subscribe to the console broadcast channel.
    let mut rx = console_tx.subscribe();

    let _ = socket
        .send(Message::Text(
            serde_json::json!({
                "level": "info",
                "message": "console streaming connected"
            })
            .to_string()
            .into(),
        ))
        .await;

    // Forward console messages to the WebSocket client.
    loop {
        tokio::select! {
            Ok(msg) = rx.recv() => {
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
}
