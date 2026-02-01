use anyhow::{Context, Result};
use clap::{Parser, Subcommand};

mod client;
mod output;

#[derive(Parser)]
#[command(
    name = "tauri-browser",
    version,
    about = "CLI for automating Tauri apps"
)]
struct Cli {
    /// Debug bridge port (default: 9229)
    #[arg(short, long, default_value_t = 9229, global = true)]
    port: u16,

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

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("tauri_browser=info".parse()?),
        )
        .init();

    let cli = Cli::parse();
    let client = client::BridgeClient::new(cli.port);

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
