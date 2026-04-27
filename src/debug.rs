use crate::{color, config::Config};

const TOKEN_SUFFIX_LENGTH: usize = 5;
const TOKEN_LENGTH: usize = 40;

// Print a debug statement if in verbose mode
/// Prints a formatted debug block to stdout when verbose mode is enabled in config or via the CLI flag.
pub fn maybe_print(config: &Config, text: &str) {
    if config.verbose.unwrap_or_default() || config.args.verbose {
        print(text)
    }
}

// Print config with token redacted to console if in verbose mode
// Everything but the last 5 characters are turned into x's before being printed to console
/// Prints the current config to stdout in verbose mode with the API token partially redacted (all but the last 5 characters replaced with `x`).
pub fn maybe_print_redacted_config(config: &Config) {
    if config.verbose.unwrap_or_default() || config.args.verbose {
        let token = config.token.as_ref().map(|t| {
            let redacted = "x".repeat(TOKEN_LENGTH - TOKEN_SUFFIX_LENGTH);
            let suffix = t.len().saturating_sub(TOKEN_SUFFIX_LENGTH);
            format!("{}{}", redacted, &t[suffix..])
        });
        let mut config = config.clone();
        config.token = token;
        print(&format!("{config:#?}"));
    }
}

// Print a debug statement
/// Unconditionally prints a formatted debug block (`=== DEBUG === … ===`) to stdout.
pub fn print(text: &str) {
    let text = format!("=== DEBUG ===\n{text}\n===");
    let text = color::debug_string(&text);

    println!("{text}");
}
