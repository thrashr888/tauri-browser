# tauri-browser

A CLI + Tauri plugin for automating and inspecting Tauri apps.

## Install

```sh
cargo install tauri-browser
```

## Usage

Add the plugin to your Tauri app behind a feature flag:

```toml
# Cargo.toml
[dependencies]
tauri-plugin-debug-bridge = { version = "0.1", optional = true }

[features]
debug-bridge = ["tauri-plugin-debug-bridge"]
```

```rust
// src-tauri/src/lib.rs
#[cfg(feature = "debug-bridge")]
app.handle().plugin(tauri_plugin_debug_bridge::init())?;
```

Add the permission to `capabilities/default.json`:

```json
"debug-bridge:default"
```

Run your app with the feature enabled:

```sh
cargo tauri dev --features debug-bridge
```

Then use the CLI:

```sh
tauri-browser connect          # verify connection
tauri-browser snapshot -i      # interactive DOM snapshot
tauri-browser run-js "document.title"
tauri-browser click "@e3"      # click by ref from snapshot
tauri-browser windows          # list app windows
tauri-browser screenshot out.png
```

## Architecture

```
tauri-browser (CLI)  ◄──── HTTP/WS ────►  tauri-plugin-debug-bridge (in-app)
                         localhost:9229
```

The plugin starts a local HTTP+WS server inside your Tauri app.
The CLI talks to it. No app code changes needed beyond plugin registration.

## License

MIT
