use crate::{config::Config, format};
const TOKEN_PREFIX_LENGTH: usize = 4;

// Print a debug statement if in verbose mode
pub fn maybe_print(config: &Config, text: &str) {
    if config.verbose.unwrap_or_default() || config.args.verbose {
        print(text);
    }
}

// Print config with token redacted when in verbose mode.
pub fn maybe_print_redacted_config(config: &Config) {
    if config.verbose.unwrap_or_default() || config.args.verbose {
        let token = config.token.as_ref().map(|token| redact_token(token));
        let mut redacted_config = config.clone();
        redacted_config.token = token;
        print(&format!("{redacted_config:#?}"));
    }
}

fn redact_token(token: &str) -> String {
    let visible: String = token.chars().take(TOKEN_PREFIX_LENGTH).collect();
    let redacted_length = token.chars().count().saturating_sub(TOKEN_PREFIX_LENGTH);
    format!("{visible}{}", "x".repeat(redacted_length))
}

// Print a debug statement
pub fn print(text: &str) {
    let text = format!("=== DEBUG ===\n{text}\n===");
    let text = format::debug_string(&text);

    println!("{text}");
}
