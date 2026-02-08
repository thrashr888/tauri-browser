# tauri-browser

A CLI + Tauri plugin for automating and inspecting Tauri apps. Ref-based element targeting with output designed for LLM consumption.

## Install the Skill

Add the [Claude Code](https://docs.anthropic.com/en/docs/claude-code) skill to your project:

```sh
npx skills add thrashr888/tauri-browser
```

Or install globally:

```sh
npx skills add thrashr888/tauri-browser -g
```

## Install the CLI

```sh
cargo install tauri-browser
```

## Setup

Add the plugin to your Tauri app behind a feature flag:

```toml
# Cargo.toml
[dependencies]
tauri-plugin-debug-bridge = { version = "0.2", optional = true }

[features]
debug-bridge = ["tauri-plugin-debug-bridge"]
```

```rust
// src-tauri/src/lib.rs
#[cfg(feature = "debug-bridge")]
app.plugin(tauri_plugin_debug_bridge::init());
```

Add the permission to `capabilities/default.json`:

```json
"debug-bridge:default"
```

Run your app with the feature enabled:

```sh
cargo tauri dev --features debug-bridge
```

## Authentication

The plugin generates a random auth token on each startup and writes a discovery file to `/tmp/tauri-debug-bridge/<app-identifier>.json`. The CLI reads this automatically — no token needed in your commands:

```sh
tauri-browser connect              # auto-discovers token
tauri-browser snapshot -i          # just works
```

When multiple Tauri apps are running, specify which one:

```sh
tauri-browser --app com.example.myapp connect
```

You can still pass the token explicitly if needed:

```sh
export TAURI_BROWSER_TOKEN="a1b2c3d4e5f6..."
tauri-browser connect

# Or per-command:
tauri-browser --token "a1b2c3d4e5f6..." connect
```

The `/health` endpoint does not require auth.

## Usage

```sh
tauri-browser connect                        # verify connection
tauri-browser snapshot -i                    # interactive elements with @refs
tauri-browser click "@e3"                    # click by ref
tauri-browser fill "@e2" "user@example.com"  # fill input by ref
tauri-browser run-js "document.title"        # execute JS
tauri-browser screenshot out.png             # capture screenshot
tauri-browser windows                        # list app windows
tauri-browser invoke get_data '{"id":1}'     # call Tauri commands
tauri-browser events emit "refresh" '{}'     # emit events
tauri-browser console                        # stream JS console output
tauri-browser logs --level warn              # stream Rust logs
```

## Architecture

```
tauri-browser (CLI)  ◄──── HTTP/WS ────►  tauri-plugin-debug-bridge (in-app)
                         localhost:9229
```

The plugin starts a local HTTP+WS server inside your Tauri app.
The CLI talks to it. No app code changes needed beyond plugin registration.

## Troubleshooting

**401 Unauthorized on all requests**
The CLI auto-discovers the token from `/tmp/tauri-debug-bridge/`. If that fails, set `TAURI_BROWSER_TOKEN` to the token printed at app startup. The token changes every restart.

**Eval/invoke times out after 10-30s**
The `debug-bridge:default` permission must be in your `capabilities/default.json`. Without it, Tauri silently blocks the `eval_callback` command and results never return.

**Console streaming shows nothing**
Make sure you're on plugin version 0.2.5+ which includes `console_callback` in the default permission set. Earlier versions only permitted `eval_callback`.

**Port already in use**
Configure a different port (or `0` for auto-assign) in `tauri.conf.json`:
```json
{
  "plugins": {
    "debug-bridge": {
      "port": 0
    }
  }
}
```

The CLI discovers the actual port from the discovery file automatically.

## License

MIT
