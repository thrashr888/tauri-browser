use serde::Serialize;

/// Output format for CLI responses.
#[derive(Clone, Debug, clap::ValueEnum)]
pub enum Format {
    /// Human-readable text (default)
    Text,
    /// JSON output for programmatic consumption
    Json,
}

impl std::fmt::Display for Format {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Format::Text => write!(f, "text"),
            Format::Json => write!(f, "json"),
        }
    }
}

/// Print a serializable value in the requested format.
pub fn print(value: &impl Serialize, format: &Format) {
    match format {
        Format::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(value)
                    .unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}")),
            );
        }
        Format::Text => {
            // For text format, use a compact representation.
            // Specific commands can override this with custom formatting.
            let json = serde_json::to_value(value).unwrap_or(serde_json::Value::Null);
            print_value(&json, 0);
        }
    }
}

fn print_value(value: &serde_json::Value, indent: usize) {
    let pad = "  ".repeat(indent);
    match value {
        serde_json::Value::Object(map) => {
            for (k, v) in map {
                match v {
                    serde_json::Value::Object(_) | serde_json::Value::Array(_) => {
                        println!("{pad}{k}:");
                        print_value(v, indent + 1);
                    }
                    _ => {
                        println!("{pad}{k}: {}", format_scalar(v));
                    }
                }
            }
        }
        serde_json::Value::Array(arr) => {
            for item in arr {
                print_value(item, indent);
                if indent == 0 {
                    println!();
                }
            }
        }
        _ => println!("{pad}{}", format_scalar(value)),
    }
}

fn format_scalar(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Null => "null".to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Number(n) => n.to_string(),
        other => other.to_string(),
    }
}
