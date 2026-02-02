const COMMANDS: &[&str] = &["eval_callback"];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}
