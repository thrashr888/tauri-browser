# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**tauri-browser** — A CLI + Tauri plugin for automating and inspecting Tauri apps, designed for Claude Code agent integration. Enables remote debugging, DOM interaction, JavaScript execution, and state inspection.

Two crates in a Cargo workspace:
- `tauri-plugin-debug-bridge` — Tauri plugin that starts an HTTP+WS server (default port 9229)
- `tauri-browser` — CLI that talks to the plugin, same UX as agent-browser

## Build Commands

```bash
# Check everything compiles
cargo check

# Quality gates (run before commits)
cargo fmt -- --check && cargo clippy -- -D warnings

# Build release
cargo build --release

# Install CLI locally
cargo install --path crates/tauri-browser
```

## Architecture

```
crates/
├── tauri-plugin-debug-bridge/   # Tauri plugin (lib crate)
│   └── src/
│       ├── lib.rs               # Plugin init, axum router, IPC result channel
│       ├── webview.rs           # JS execution, screenshot, snapshot, click, fill
│       ├── backend.rs           # invoke proxy, windows, config
│       ├── events.rs            # event emit/list
│       └── logs.rs              # WebSocket log/console streaming
└── tauri-browser/               # CLI (bin crate)
    └── src/
        ├── main.rs              # Clap arg parsing, command dispatch
        ├── client.rs            # HTTP/WS client to debug bridge
        └── output.rs            # Text/JSON output formatting
```

Key pattern: The plugin uses an IPC result channel — injected JS calls `plugin:debug-bridge|eval_callback` to return results from webview operations. This is necessary because Tauri's `WebviewWindow` API is fire-and-forget.

## Important: Axum Layer Ordering

In axum/tower, the LAST `.layer()` call is the outermost middleware (runs first). This means:
```rust
Router::new()
    .layer(middleware::from_fn(auth_middleware))  // inner: reads extensions
    .layer(axum::Extension(auth_token))          // outer: sets extensions first
```
If the order is swapped, `auth_middleware` runs before the Extension is set and auth breaks silently.

## Plugin Permissions

The plugin registers two Tauri commands: `eval_callback` and `console_callback`. Both must be listed in:
1. `build.rs` `COMMANDS` array — generates the permission definitions
2. `permissions/default.toml` — includes them in the default permission set
3. The consuming app's `capabilities/default.json` — must include `"debug-bridge:default"`

If any of these are missing, Tauri silently blocks the command — no error, just a timeout. This is the #1 cause of "eval timed out" bugs.

## Publishing

Both crates are published to crates.io via trusted publishing (GitHub Actions OIDC). Pushing a `v*` tag triggers the release workflow. Just bump the version and tag.

When bumping versions, **always commit `Cargo.lock` together with `Cargo.toml` changes**. This is a workspace with a binary crate — the lockfile must stay in sync.

## Beads Issue Tracking

This repository uses `bd` (beads) for issue tracking.

```bash
bd ready                    # Show unblocked work
bd list --status=open       # All open issues
bd update <id> --status=in_progress
bd close <id>
bd sync                     # Sync at session end
```
