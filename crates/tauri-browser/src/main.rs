use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use std::path::Path;

mod client;
mod output;

/// Well-known directory where the plugin writes discovery files.
const DISCOVERY_DIR: &str = "/tmp/tauri-debug-bridge";

#[derive(Parser)]
#[command(
    name = "tauri-browser",
    version,
    about = "CLI for automating Tauri apps"
)]
struct Cli {
    /// Debug bridge port (overrides discovery)
    #[arg(short, long, global = true)]
    port: Option<u16>,

    /// App identifier to connect to (reads from discovery file)
    #[arg(short = 'a', long, global = true)]
    app: Option<String>,

    /// Auth token (overrides discovery)
    #[arg(short = 't', long, global = true, env = "TAURI_BROWSER_TOKEN")]
    token: Option<String>,

    /// Output format
    #[arg(short, long, default_value = "text", global = true)]
    format: output::Format,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Check connection to debug bridge
    Connect,

    /// Capture webview screenshot
    Screenshot {
        /// Save to file instead of stdout
        path: Option<String>,
    },

    /// Dump DOM accessibility tree with element refs
    Snapshot {
        /// Only show interactive elements
        #[arg(short, long)]
        interactive: bool,
    },

    /// Click an element by @ref or CSS selector
    Click {
        /// Element ref (@e1) or CSS selector
        selector: String,
    },

    /// Fill an input element with text
    Fill {
        /// Element ref (@e1) or CSS selector
        selector: String,
        /// Text to fill
        text: String,
    },

    /// Execute JavaScript in the webview
    RunJs {
        /// JavaScript code to execute
        code: String,
    },

    /// View console output
    Console,

    /// View JavaScript errors
    Errors,

    /// Call a registered Tauri command
    Invoke {
        /// Command name
        command: String,
        /// JSON arguments
        args: Option<String>,
    },

    /// Dump managed state
    State,

    /// List registered Tauri commands
    Commands,

    /// Work with Tauri events
    Events {
        #[command(subcommand)]
        action: EventAction,
    },

    /// Stream Rust-side logs
    Logs {
        /// Minimum log level
        #[arg(long, default_value = "info")]
        level: String,
    },

    /// List open windows
    Windows,
}

#[derive(Subcommand)]
enum EventAction {
    /// Emit an event
    Emit {
        /// Event name
        name: String,
        /// JSON payload
        payload: Option<String>,
    },
    /// Listen for events (streams via WebSocket)
    Listen {
        /// Event name
        name: String,
    },
    /// List known events
    List,
}

/// Read port and token from a discovery file written by the plugin.
fn read_discovery_file(path: &Path) -> Option<(u16, String)> {
    let content = std::fs::read_to_string(path).ok()?;
    let json: serde_json::Value = serde_json::from_str(&content).ok()?;
    let port = json["port"].as_u64()? as u16;
    let token = json["token"].as_str()?.to_string();
    Some((port, token))
}

/// Resolve connection parameters from CLI flags or discovery files.
fn resolve_connection(cli: &Cli) -> Result<(u16, Option<String>)> {
    // Explicit token provided — use manual mode.
    if cli.token.is_some() {
        return Ok((cli.port.unwrap_or(9229), cli.token.clone()));
    }

    // Try discovery from /tmp/tauri-debug-bridge/.
    let dir = Path::new(DISCOVERY_DIR);

    if let Some(app_id) = &cli.app {
        // Target a specific app.
        let path = dir.join(format!("{app_id}.json"));
        if let Some((port, token)) = read_discovery_file(&path) {
            let port = cli.port.unwrap_or(port);
            return Ok((port, Some(token)));
        }
        bail!("no discovery file for app '{app_id}' at {}", path.display());
    }

    // No --app: scan directory for available apps.
    if let Ok(entries) = std::fs::read_dir(dir) {
        let files: Vec<_> = entries
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map(|x| x == "json").unwrap_or(false))
            .collect();

        if files.len() == 1 {
            if let Some((port, token)) = read_discovery_file(&files[0].path()) {
                let port = cli.port.unwrap_or(port);
                return Ok((port, Some(token)));
            }
        } else if files.len() > 1 {
            eprintln!("Multiple apps detected. Use --app to specify:");
            for f in &files {
                if let Some(name) = f.path().file_stem() {
                    eprintln!("  --app {}", name.to_string_lossy());
                }
            }
            bail!("multiple apps running — specify --app <identifier>");
        }
    }

    // No discovery files found — fall back to defaults.
    Ok((cli.port.unwrap_or(9229), None))
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("tauri_browser=info".parse()?),
        )
        .init();

    let cli = Cli::parse();
    let (port, token) = resolve_connection(&cli)?;
    let client = client::BridgeClient::new(port, token.as_deref());

    match cli.command {
        Command::Connect => {
            let health = client.health().await?;
            output::print(&health, &cli.format);
        }
        Command::Screenshot { path } => {
            let data = client.screenshot().await?;
            if let Some(path) = path {
                std::fs::write(&path, &data)
                    .with_context(|| format!("writing screenshot to {path}"))?;
                println!("Screenshot saved to {path}");
            } else {
                // Write raw PNG to stdout for piping
                use std::io::Write;
                std::io::stdout().write_all(&data)?;
            }
        }
        Command::Snapshot { interactive } => {
            let snapshot = client.snapshot(interactive).await?;
            output::print(&snapshot, &cli.format);
        }
        Command::Click { selector } => {
            let result = client.click(&selector).await?;
            output::print(&result, &cli.format);
        }
        Command::Fill { selector, text } => {
            let result = client.fill(&selector, &text).await?;
            output::print(&result, &cli.format);
        }
        Command::RunJs { code } => {
            let result = client.run_js(&code).await?;
            output::print(&result, &cli.format);
        }
        Command::Console => {
            client.stream_console().await?;
        }
        Command::Errors => {
            client.stream_errors().await?;
        }
        Command::Invoke { command, args } => {
            let args = args.as_deref().unwrap_or("{}");
            let result = client.invoke(&command, args).await?;
            output::print(&result, &cli.format);
        }
        Command::State => {
            let state = client.state().await?;
            output::print(&state, &cli.format);
        }
        Command::Commands => {
            let cmds = client.commands().await?;
            output::print(&cmds, &cli.format);
        }
        Command::Events { action } => match action {
            EventAction::Emit { name, payload } => {
                let payload = payload.as_deref().unwrap_or("{}");
                let result = client.event_emit(&name, payload).await?;
                output::print(&result, &cli.format);
            }
            EventAction::Listen { name } => {
                client.event_listen(&name).await?;
            }
            EventAction::List => {
                let events = client.event_list().await?;
                output::print(&events, &cli.format);
            }
        },
        Command::Logs { level } => {
            client.stream_logs(&level).await?;
        }
        Command::Windows => {
            let windows = client.windows().await?;
            output::print(&windows, &cli.format);
        }
    }

    Ok(())
}
