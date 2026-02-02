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
tauri-browser logs --level warn              # stream logs
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
