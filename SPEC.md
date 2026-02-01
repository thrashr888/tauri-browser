## tauri-browser

**A CLI + Tauri plugin for automating and inspecting Tauri apps, designed for Claude Code.**

### Architecture

```
┌─────────────┐         HTTP/WS          ┌──────────────────────────┐
│ tauri-browser│ ◄──────────────────────► │ tauri-plugin-debug-bridge│
│   (CLI)      │     localhost:9229       │   (runs inside app)      │
└─────────────┘                          │                          │
      │                                  │  ┌─────────┐ ┌────────┐ │
      │  Claude Code                     │  │ WebView │ │  Rust  │ │
      │  skill calls                     │  │  JS eval│ │ backend│ │
      │  CLI commands                    │  └─────────┘ └────────┘ │
      ▼                                  └──────────────────────────┘
  SKILL.md
  (agent-browser-style commands)
```

### Part 1: `tauri-plugin-debug-bridge` (Rust)

A dev-only Tauri plugin that exposes a local HTTP + WebSocket server on a configurable port. Added to `Cargo.toml` behind a `debug` feature flag so it's stripped from release builds.

**Webview commands** (via `WebviewWindow::eval()`):
- `eval <js>` — execute arbitrary JS, return result
- `snapshot` — dump DOM accessibility tree (same format as agent-browser)
- `screenshot` — capture webview contents as PNG
- `click <selector>` / `fill <selector> <text>` — DOM interaction via injected JS
- `console` — stream `console.log/warn/error` back (hook early via injected script)

**Rust backend commands:**
- `invoke <cmd> <args>` — call any registered Tauri command directly and return the result
- `state` — dump managed state (requires `Debug` trait on state types)
- `events list` — list registered event listeners
- `events emit <name> <payload>` — emit a Tauri event
- `events listen <name>` — subscribe to events via WebSocket stream
- `logs` — stream Rust-side `log` crate output (tap into `tracing` subscriber or `log` facade)

**App introspection:**
- `commands` — list all registered invoke commands (from `generate_handler!`)
- `windows` — list open windows/webviews
- `menu` — dump tray/menu structure
- `config` — return `tauri.conf.json` contents

### Part 2: `tauri-browser` (Rust CLI)

Same UX as agent-browser — short commands, ref-based element targeting, output designed for LLM consumption.

```bash
tauri-browser connect [port]          # Connect to debug bridge (default 9229)
tauri-browser screenshot              # Capture webview
tauri-browser screenshot path.png     # Save to file
tauri-browser snapshot -i             # Interactive elements with @refs
tauri-browser click @e1               # Click element
tauri-browser fill @e2 "text"         # Fill input
tauri-browser run-js "document.title" # Run JS in webview
tauri-browser console                 # View console output
tauri-browser errors                  # View JS errors

# Rust backend (the differentiator)
tauri-browser invoke get_signals '{"configPath":"config/live.toml"}'
tauri-browser invoke auth_status '{"production":true}'
tauri-browser state                   # Dump managed state
tauri-browser commands                # List available commands
tauri-browser events emit "tray-refresh" '{}'
tauri-browser events listen "tray-settings-changed"
tauri-browser logs                    # Stream Rust logs
tauri-browser logs --level warn       # Filter by level
```

### Part 3: Claude Code Skill (`SKILL.md`)

Same pattern as the agent-browser skill — documents all commands, provides examples, gets loaded when Claude needs to interact with the app.

```bash
tauri-browser connect
tauri-browser snapshot -i
# Output: button "Refresh" [ref=e1], checkbox "Auto-refresh" [ref=e2], ...
tauri-browser click @e1
tauri-browser screenshot
tauri-browser console               # Check for errors after click
tauri-browser invoke get_positions '{"configPath":"config/live.toml","production":true}'
```

### Key Design Decisions

**Why a plugin, not external?**
On macOS, WKWebView doesn't expose CDP. The only way to run JS or screenshot the webview is from inside the process. A plugin gets full access to `WebviewWindow` and the Rust side.

**Why HTTP+WS, not stdin/stdout?**
Multiple CLI invocations can share one connection. WebSocket enables streaming (console logs, events). HTTP makes individual commands simple.

**Dev-only by default:**
```toml
# App's Cargo.toml
[dependencies]
tauri-plugin-debug-bridge = { version = "0.1", optional = true }

[features]
debug = ["tauri-plugin-debug-bridge"]
```
```rust
// lib.rs
#[cfg(feature = "debug")]
app.plugin(tauri_plugin_debug_bridge::init(9229));
```

**Accessibility tree via JS injection:**
The snapshot command injects a script that walks the DOM and builds the same ref-based tree that agent-browser uses. This means the skill instructions are nearly identical — if you know agent-browser, you know tauri-browser.

### Crate Structure

```
tauri-browser/
├── crates/
│   ├── tauri-plugin-debug-bridge/    # The Tauri plugin (lib)
│   │   ├── src/
│   │   │   ├── lib.rs                # Plugin init, HTTP/WS server
│   │   │   ├── webview.rs            # JS execution, screenshot, DOM snapshot
│   │   │   ├── backend.rs            # invoke, state, command listing
│   │   │   ├── events.rs             # Event emit/listen/list
│   │   │   └── logs.rs               # Log capture + streaming
│   │   └── Cargo.toml
│   └── tauri-browser/                # The CLI (bin)
│       ├── src/
│       │   ├── main.rs               # Arg parsing, command dispatch
│       │   ├── client.rs             # HTTP/WS client to debug bridge
│       │   └── output.rs             # Formatting (compact, json, refs)
│       └── Cargo.toml
├── skill/
│   └── SKILL.md                      # Claude Code skill
├── Cargo.toml                        # Workspace
└── README.md
```

### Dependencies

- **Plugin**: `axum` (HTTP/WS server), `tokio`, `serde_json`, `tracing-subscriber` (log capture)
- **CLI**: `clap` (args), `reqwest` (HTTP client), `tokio-tungstenite` (WS), `image` (PNG handling)
