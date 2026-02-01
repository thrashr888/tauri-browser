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
tauri-browser events listen "state-changed"  # Stream events
tauri-browser events list
```

### Logs and console

```bash
tauri-browser logs                       # Stream Rust logs
tauri-browser logs --level warn          # Filter by level
tauri-browser console                    # Stream JS console output
tauri-browser errors                     # Stream JS errors
```

## Typical workflow

```bash
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
