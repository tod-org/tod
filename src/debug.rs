use crate::{config::Config, format};
const TOKEN_SUFFIX_LENGTH: usize = 5;

// Print a debug statement if in verbose mode
pub fn maybe_print(config: &Config, text: &str) {
    if config.verbose.unwrap_or_default() || config.args.verbose {
        print(text);
    }
}

// Print config with token redacted when in verbose mode.
pub fn maybe_print_redacted_config(config: &Config) {
    if config.verbose.unwrap_or_default() || config.args.verbose {
        let token = config.token.as_ref().map(|token| {
            let prefix = "x".repeat(token.len().saturating_sub(TOKEN_SUFFIX_LENGTH));
            let suffix = &token[token.len().saturating_sub(TOKEN_SUFFIX_LENGTH)..];
            format!("{prefix}{suffix}")
        });
        let mut redacted_config = config.clone();
        redacted_config.token = token;
        print(&format!("{redacted_config:#?}"));
    }
}

// Print a debug statement
pub fn print(text: &str) {
    let text = format!("=== DEBUG ===\n{text}\n===");
    let text = format::debug_string(&text);

    println!("{text}");
}
