---
name: tauri-browser
description: Automate and inspect Tauri apps via CLI. Ref-based element targeting, DOM snapshots, screenshots, JS execution, Tauri command invocation, and log streaming. Output designed for LLM consumption.
---

# tauri-browser

Automate and inspect Tauri apps via the debug bridge plugin. Same UX as agent-browser — ref-based element targeting, output designed for LLM consumption.

## Prerequisites

The target Tauri app must include `tauri-plugin-debug-bridge` with the `debug` feature enabled:

```toml
# App's Cargo.toml
[dependencies]
tauri-plugin-debug-bridge = { version = "0.1", optional = true }

[features]
debug = ["tauri-plugin-debug-bridge"]
```

```rust
// App's lib.rs
#[cfg(feature = "debug")]
app.plugin(tauri_plugin_debug_bridge::init());
```

Run the app with `cargo run --features debug`.

## Authentication

The plugin generates a random auth token on each startup, printed to stdout:

```
debug-bridge auth token: a1b2c3d4e5f6...
```

Pass it to the CLI via `--token` or the `TAURI_BROWSER_TOKEN` env var:

```bash
export TAURI_BROWSER_TOKEN="a1b2c3d4e5f6..."
tauri-browser connect

# Or per-command:
tauri-browser --token "a1b2c3d4e5f6..." connect
```

The `/health` endpoint does not require auth.

## Commands

### Connect and inspect

```bash
tauri-browser connect                    # Check connection (default port 9229)
tauri-browser -p 9230 connect            # Connect on custom port
tauri-browser windows                    # List open windows
```

### DOM interaction (ref-based)

```bash
tauri-browser snapshot -i                # Interactive elements with @refs
# Output: button "Submit" [ref=@e1], input "Email" [ref=@e2], ...

tauri-browser click @e1                  # Click by ref
tauri-browser fill @e2 "user@example.com" # Fill input by ref
tauri-browser click "button.submit"      # Click by CSS selector
```

### Screenshots

```bash
tauri-browser screenshot                 # PNG to stdout
tauri-browser screenshot app.png         # Save to file
```

### JavaScript execution

```bash
tauri-browser run-js "document.title"    # Run JS, get result
tauri-browser run-js "document.querySelectorAll('li').length"
```

### Tauri backend

```bash
tauri-browser invoke get_signals '{"configPath":"config/live.toml"}'
tauri-browser invoke auth_status '{}'
tauri-browser state                      # Dump managed state
tauri-browser commands                   # List registered commands
```

### Events

```bash
tauri-browser events emit "refresh" '{"force":true}'
tauri-browser events listen "state-changed"  # Stream events via WebSocket
```

### Console

```bash
tauri-browser console                    # Stream JS console output (log/warn/error/info)
tauri-browser errors                     # Stream JS errors (alias for console)
```

## Typical workflow

```bash
export TAURI_BROWSER_TOKEN="<token from app startup>"
tauri-browser connect
tauri-browser snapshot -i
# See: button "Refresh" [ref=@e1], input "Search" [ref=@e2]
tauri-browser fill @e2 "AAPL"
tauri-browser click @e1
tauri-browser screenshot result.png
tauri-browser console                    # Check for errors
tauri-browser invoke get_positions '{"symbol":"AAPL"}'
```

## Output formats

```bash
tauri-browser -f json snapshot           # JSON output
tauri-browser -f text connect            # Human-readable (default)
```
