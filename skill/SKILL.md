---
name: tauri-browser
description: Automate and inspect Tauri apps via CLI. Ref-based element targeting, DOM snapshots, screenshots, JS execution, Tauri command invocation, and log streaming. Output designed for LLM consumption.
---

# tauri-browser

Automate and inspect Tauri apps via the debug bridge plugin. Same UX as agent-browser — ref-based element targeting, output designed for LLM consumption.

## Prerequisites

The target Tauri app must include `tauri-plugin-debug-bridge` behind a feature flag:

```toml
# App's Cargo.toml
[dependencies]
tauri-plugin-debug-bridge = { version = "0.4", optional = true }

[features]
debug-bridge = ["tauri-plugin-debug-bridge"]
```

```rust
// App's lib.rs
#[cfg(feature = "debug-bridge")]
app.plugin(tauri_plugin_debug_bridge::init());
```

Add the capability permission. Create `capabilities/debug-bridge.json` (separate file avoids overwrite if `default.json` is generated):

```json
{
  "identifier": "debug-bridge",
  "description": "Debug bridge for tauri-browser automation",
  "windows": ["main"],
  "permissions": ["debug-bridge:default"]
}
```

Without this, eval/invoke will silently time out.

Run the app with `cargo tauri dev --features debug-bridge`.

## Authentication

The plugin writes a discovery file on startup to `/tmp/tauri-debug-bridge/<app-identifier>.json` containing the port and auth token. The CLI reads this automatically:

```bash
tauri-browser connect               # auto-discovers token and port
tauri-browser snapshot -i           # just works, no token needed
```

When multiple Tauri apps are running, specify which one:

```bash
tauri-browser --app com.example.myapp connect
```

You can still pass the token explicitly as an override:

```bash
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
tauri-browser connect                    # Auto-discovers token
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
