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

## Publishing

Both crates are published to crates.io via trusted publishing (GitHub Actions OIDC). Pushing to `main` auto-publishes if the version in `Cargo.toml` changed. No manual `cargo publish` needed — just bump the version and push.

## Beads Issue Tracking

This repository uses `bd` (beads) for issue tracking.

```bash
bd ready                    # Show unblocked work
bd list --status=open       # All open issues
bd update <id> --status=in_progress
bd close <id>
bd sync                     # Sync at session end
```
