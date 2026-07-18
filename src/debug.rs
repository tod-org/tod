use crate::{config::Config, format};

// Print a debug statement if in verbose mode
pub fn maybe_print(config: &Config, text: &str) {
    if config.verbose.unwrap_or_default() || config.args.verbose {
        print(text);
    }
}

// Print a debug statement
pub fn print(text: &str) {
    let text = format!("=== DEBUG ===\n{text}\n===");
    let text = format::debug_string(&text);

    println!("{text}");
}
