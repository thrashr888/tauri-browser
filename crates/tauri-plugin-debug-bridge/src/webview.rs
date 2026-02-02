use std::sync::Arc;
use std::time::Duration;

use axum::{
    extract::State,
    http::StatusCode,
    response::{Json, Response},
};
use serde::{Deserialize, Serialize};
use tauri::{Manager, Runtime, WebviewWindow};
use tokio::sync::{Mutex, oneshot};

use crate::{BridgeState, EvalResult};

#[derive(Deserialize)]
pub struct EvalRequest {
    pub js: String,
    /// Optional window label. Defaults to "main".
    pub window: Option<String>,
}

#[derive(Deserialize)]
pub struct ClickRequest {
    pub selector: String,
    pub window: Option<String>,
}

#[derive(Deserialize)]
pub struct FillRequest {
    pub selector: String,
    pub text: String,
    pub window: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct SnapshotElement {
    pub tag: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r#ref: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    pub interactive: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<SnapshotElement>,
}

#[derive(Serialize, Deserialize)]
pub struct SnapshotResponse {
    pub title: String,
    pub url: String,
    pub elements: Vec<SnapshotElement>,
}

fn get_window<R: Runtime>(
    app: &tauri::AppHandle<R>,
    label: Option<&str>,
) -> Result<WebviewWindow<R>, (StatusCode, String)> {
    let label = label.unwrap_or("main");
    app.get_webview_window(label)
        .ok_or_else(|| (StatusCode::NOT_FOUND, format!("window '{label}' not found")))
}

/// Inject JS that evaluates code and sends the result back via the plugin's
/// `eval_callback` Tauri command. Returns the result via a oneshot channel.
async fn eval_with_result<R: Runtime>(
    state: &BridgeState<R>,
    window: &WebviewWindow<R>,
    js_code: &str,
) -> Result<EvalResult, (StatusCode, String)> {
    let id = uuid_v4();
    let (tx, rx) = oneshot::channel();

    {
        let mut pending = state.pending.lock().await;
        pending.insert(id.clone(), tx);
    }

    // Wrap the user's JS so it evaluates and calls back with the result.
    // Use __TAURI_INTERNALS__ which is always available in the Tauri webview,
    // unlike window.__TAURI__ which requires the @tauri-apps/api import.
    //
    // For single-line expressions (like `document.title`), auto-add `return`.
    // For multi-line code or code with statement keywords, pass through as-is
    // (callers must use `return` explicitly).
    // Note: we avoid eval() since webview CSP may not include 'unsafe-eval'.
    let code_body = if looks_like_expression(js_code) {
        format!("return ({js_code})")
    } else {
        js_code.to_string()
    };

    let wrapped = format!(
        r#"(async () => {{
            try {{
                const __result = await (async () => {{ {code} }})();
                await window.__TAURI_INTERNALS__.invoke(
                    'plugin:debug-bridge|eval_callback',
                    {{ id: '{id}', success: true, value: __result, error: null }}
                );
            }} catch(__e) {{
                await window.__TAURI_INTERNALS__.invoke(
                    'plugin:debug-bridge|eval_callback',
                    {{ id: '{id}', success: false, value: null, error: __e.toString() }}
                );
            }}
        }})()"#,
        code = code_body,
        id = id,
    );

    window
        .eval(&wrapped)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Wait for result with timeout.
    match tokio::time::timeout(Duration::from_secs(10), rx).await {
        Ok(Ok(result)) => Ok(result),
        Ok(Err(_)) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            "eval callback channel dropped".to_string(),
        )),
        Err(_) => {
            // Clean up the pending entry.
            let mut pending = state.pending.lock().await;
            pending.remove(&id);
            Err((
                StatusCode::GATEWAY_TIMEOUT,
                "eval timed out after 10s".to_string(),
            ))
        }
    }
}

/// POST /eval — execute JS in the webview and return the result.
pub async fn webview_eval<R: Runtime>(
    State(state): State<Arc<Mutex<BridgeState<R>>>>,
    Json(req): Json<EvalRequest>,
) -> Result<Json<EvalResult>, (StatusCode, String)> {
    let state = state.lock().await;
    let window = get_window(&state.app, req.window.as_deref())?;
    let result = eval_with_result(&state, &window, &req.js).await?;
    Ok(Json(result))
}

/// GET /screenshot — capture the webview as a base64-encoded PNG.
pub async fn screenshot<R: Runtime>(
    State(state): State<Arc<Mutex<BridgeState<R>>>>,
) -> Result<Response, (StatusCode, String)> {
    let state = state.lock().await;
    let window = get_window(&state.app, None)?;

    // Capture via canvas: render the document to a canvas and export as PNG data URL.
    let js = r#"
        return await new Promise((resolve, reject) => {
            try {
                const canvas = document.createElement('canvas');
                const rect = document.documentElement.getBoundingClientRect();
                canvas.width = window.innerWidth * window.devicePixelRatio;
                canvas.height = window.innerHeight * window.devicePixelRatio;
                const ctx = canvas.getContext('2d');
                ctx.scale(window.devicePixelRatio, window.devicePixelRatio);

                // Use html2canvas-lite approach: serialize to SVG foreignObject
                const data = new XMLSerializer().serializeToString(document.documentElement);
                const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="${window.innerWidth}" height="${window.innerHeight}">
                    <foreignObject width="100%" height="100%">
                        ${data}
                    </foreignObject>
                </svg>`;
                const img = new Image();
                const blob = new Blob([svg], {type: 'image/svg+xml;charset=utf-8'});
                const url = URL.createObjectURL(blob);
                img.onload = () => {
                    ctx.drawImage(img, 0, 0);
                    URL.revokeObjectURL(url);
                    resolve(canvas.toDataURL('image/png'));
                };
                img.onerror = (e) => reject('Screenshot capture failed: ' + e);
                img.src = url;
            } catch(e) {
                reject(e.toString());
            }
        });
    "#;

    let result = eval_with_result(&state, &window, js).await?;

    match result.value {
        Some(serde_json::Value::String(data_url)) => {
            // Strip the data:image/png;base64, prefix
            let png_data = if let Some(b64) = data_url.strip_prefix("data:image/png;base64,") {
                use base64::Engine;
                base64::engine::general_purpose::STANDARD
                    .decode(b64)
                    .map_err(|e| {
                        (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            format!("base64 decode failed: {e}"),
                        )
                    })?
            } else {
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "unexpected screenshot format".to_string(),
                ));
            };

            Ok(axum::response::Response::builder()
                .header("Content-Type", "image/png")
                .body(axum::body::Body::from(png_data))
                .unwrap())
        }
        _ => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("screenshot failed: {}", result.error.unwrap_or_default()),
        )),
    }
}

/// GET /snapshot — dump the DOM as a ref-based accessibility tree.
pub async fn snapshot<R: Runtime>(
    State(state): State<Arc<Mutex<BridgeState<R>>>>,
) -> Result<Json<SnapshotResponse>, (StatusCode, String)> {
    let state = state.lock().await;
    let window = get_window(&state.app, None)?;

    let js = SNAPSHOT_JS;
    let result = eval_with_result(&state, &window, js).await?;

    match result.value {
        Some(val) => {
            let snapshot: SnapshotResponse = serde_json::from_value(val).map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("failed to parse snapshot: {e}"),
                )
            })?;
            Ok(Json(snapshot))
        }
        None => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("snapshot failed: {}", result.error.unwrap_or_default()),
        )),
    }
}

/// POST /click — click an element by @ref or CSS selector.
pub async fn click<R: Runtime>(
    State(state): State<Arc<Mutex<BridgeState<R>>>>,
    Json(req): Json<ClickRequest>,
) -> Result<Json<EvalResult>, (StatusCode, String)> {
    let state = state.lock().await;
    let window = get_window(&state.app, req.window.as_deref())?;

    let js = if req.selector.starts_with('@') {
        // Ref-based: find element by data-debug-ref attribute
        format!(
            r#"
            const el = document.querySelector('[data-debug-ref="{}"]');
            if (!el) throw new Error('Ref not found: {}');
            el.scrollIntoView({{block: 'center'}});
            el.click();
            return true;
            "#,
            &req.selector[1..],
            req.selector,
        )
    } else {
        format!(
            r#"
            const el = document.querySelector({});
            if (!el) throw new Error('Element not found: {}');
            el.scrollIntoView({{block: 'center'}});
            el.click();
            return true;
            "#,
            serde_json::to_string(&req.selector).unwrap(),
            req.selector,
        )
    };

    let result = eval_with_result(&state, &window, &js).await?;
    Ok(Json(result))
}

/// POST /fill — fill an input element with text.
pub async fn fill<R: Runtime>(
    State(state): State<Arc<Mutex<BridgeState<R>>>>,
    Json(req): Json<FillRequest>,
) -> Result<Json<EvalResult>, (StatusCode, String)> {
    let state = state.lock().await;
    let window = get_window(&state.app, req.window.as_deref())?;

    let text_json = serde_json::to_string(&req.text).unwrap();

    let js = if req.selector.starts_with('@') {
        format!(
            r#"
            const el = document.querySelector('[data-debug-ref="{}"]');
            if (!el) throw new Error('Ref not found: {}');
            el.scrollIntoView({{block: 'center'}});
            el.focus();
            el.value = {text};
            el.dispatchEvent(new Event('input', {{bubbles: true}}));
            el.dispatchEvent(new Event('change', {{bubbles: true}}));
            return true;
            "#,
            &req.selector[1..],
            req.selector,
            text = text_json,
        )
    } else {
        format!(
            r#"
            const el = document.querySelector({selector});
            if (!el) throw new Error('Element not found: {}');
            el.scrollIntoView({{block: 'center'}});
            el.focus();
            el.value = {text};
            el.dispatchEvent(new Event('input', {{bubbles: true}}));
            el.dispatchEvent(new Event('change', {{bubbles: true}}));
            return true;
            "#,
            req.selector,
            selector = serde_json::to_string(&req.selector).unwrap(),
            text = text_json,
        )
    };

    let result = eval_with_result(&state, &window, &js).await?;
    Ok(Json(result))
}

/// Detect if JS code is a simple expression (no statements).
/// Single-line code without statement keywords gets auto-wrapped with `return`.
fn looks_like_expression(code: &str) -> bool {
    let trimmed = code.trim();
    if trimmed.contains('\n') {
        return false;
    }
    let keywords = [
        "return ",
        "return;",
        "const ",
        "let ",
        "var ",
        "if ",
        "if(",
        "for ",
        "for(",
        "while ",
        "while(",
        "switch ",
        "switch(",
        "throw ",
        "try ",
        "try{",
        "class ",
        "function ",
        "async function",
    ];
    !keywords.iter().any(|kw| trimmed.starts_with(kw))
}

/// Simple UUID v4 generator (no external dep needed).
pub fn uuid_v4() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let t = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    format!("{t:032x}")
}

/// JavaScript that walks the DOM and builds a ref-based accessibility tree.
/// Same pattern as agent-browser — assigns data-debug-ref attributes to
/// interactive elements and returns a structured tree.
const SNAPSHOT_JS: &str = r#"
    return (() => {
        let refCounter = 0;

        const INTERACTIVE_TAGS = new Set([
            'A', 'BUTTON', 'INPUT', 'SELECT', 'TEXTAREA', 'DETAILS',
            'SUMMARY', 'LABEL', 'OPTION'
        ]);

        const INTERACTIVE_ROLES = new Set([
            'button', 'link', 'textbox', 'checkbox', 'radio', 'combobox',
            'listbox', 'menuitem', 'tab', 'switch', 'slider', 'spinbutton',
            'searchbox', 'option', 'menuitemcheckbox', 'menuitemradio',
            'treeitem'
        ]);

        function isInteractive(el) {
            if (INTERACTIVE_TAGS.has(el.tagName)) return true;
            const role = el.getAttribute('role');
            if (role && INTERACTIVE_ROLES.has(role)) return true;
            if (el.getAttribute('tabindex') !== null) return true;
            if (el.onclick || el.getAttribute('onclick')) return true;
            return false;
        }

        function isVisible(el) {
            if (el === document.body || el === document.documentElement) return true;
            const style = window.getComputedStyle(el);
            return style.display !== 'none' &&
                   style.visibility !== 'hidden' &&
                   style.opacity !== '0' &&
                   el.offsetParent !== null;
        }

        function getTextContent(el) {
            let text = '';
            for (const child of el.childNodes) {
                if (child.nodeType === Node.TEXT_NODE) {
                    const t = child.textContent.trim();
                    if (t) text += (text ? ' ' : '') + t;
                }
            }
            return text || null;
        }

        function walkNode(el) {
            if (el.nodeType !== Node.ELEMENT_NODE) return null;
            if (!isVisible(el)) return null;

            const tag = el.tagName.toLowerCase();

            // Skip script, style, and other non-visual elements
            if (['script', 'style', 'noscript', 'template'].includes(tag)) return null;

            const interactive = isInteractive(el);
            let ref_id = null;

            if (interactive) {
                ref_id = 'e' + (++refCounter);
                el.setAttribute('data-debug-ref', ref_id);
            }

            const children = [];
            for (const child of el.children) {
                const node = walkNode(child);
                if (node) children.push(node);
            }

            // Skip non-interactive containers with no text and only one child
            const text = getTextContent(el);
            if (!interactive && !text && children.length <= 1 && !el.getAttribute('role')) {
                return children[0] || null;
            }

            const node = {
                tag: tag,
                ref: ref_id,
                interactive: interactive,
            };

            const role = el.getAttribute('role');
            if (role) node.role = role;
            if (text) node.text = text;

            const ariaLabel = el.getAttribute('aria-label');
            const name = ariaLabel || el.getAttribute('name') || el.getAttribute('placeholder');
            if (name) node.name = name;

            if (el.value !== undefined && el.value !== '') {
                node.value = String(el.value);
            }

            if (children.length > 0) node.children = children;

            return node;
        }

        const tree = walkNode(document.body);
        return {
            title: document.title,
            url: window.location.href,
            elements: tree ? (tree.children || [tree]) : [],
        };
    })();
"#;
