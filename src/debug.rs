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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use std::io::Read;

    #[test]
    fn redact_token_keeps_first_four_chars() {
        let redacted = redact_token("abcd1234");
        assert_eq!(redacted, "abcdxxxx");
    }

    #[test]
    fn redact_token_handles_short_tokens() {
        let redacted = redact_token("abc");
        assert_eq!(redacted, "abc");
    }

    #[test]
    fn maybe_print_redacted_config_with_token_does_not_panic() {
        let mut config = Config::default();
        config.verbose = Some(true);
        config.token = Some("abcd1234".to_string());

        maybe_print_redacted_config(&config);
    }

    // These cases use gag::BufferRedirect which replaces the process-wide stdout fd.
    // They cannot run in parallel with each other (or any other test that touches
    // stdout). cargo test runs tests in threads and will fail; use cargo nextest
    // instead, which runs each test in its own process.
    #[test]
    fn buffer_redirect_tests_cannot_run_in_parallel() {
        // maybe_print suppressed when verbose is None and args.verbose is false
        {
            let mut config = Config::default();
            config.verbose = None;
            config.args.verbose = false;

            let mut buf = gag::BufferRedirect::stdout().expect("should buffer stdout");
            maybe_print(&config, "should not appear");
            let mut output = String::new();
            buf.read_to_string(&mut output)
                .expect("output should be readable");

            assert!(output.is_empty());
        }

        // maybe_print output when verbose is true
        {
            let mut config = Config::default();
            config.verbose = Some(true);
            config.args.verbose = false;

            let mut buf = gag::BufferRedirect::stdout().expect("should buffer stdout");
            maybe_print(&config, "should appear");
            let mut output = String::new();
            buf.read_to_string(&mut output)
                .expect("output should be readable");

            assert!(output.contains("should appear"));
        }

        // maybe_print output when args.verbose is true
        {
            let mut config = Config::default();
            config.verbose = None;
            config.args.verbose = true;

            let mut buf = gag::BufferRedirect::stdout().expect("should buffer stdout");
            maybe_print(&config, "should appear");
            let mut output = String::new();
            buf.read_to_string(&mut output)
                .expect("output should be readable");

            assert!(output.contains("should appear"));
        }

        // maybe_print_redacted_config suppressed when not verbose
        {
            let mut config = Config::default();
            config.verbose = None;
            config.args.verbose = false;
            config.token = Some("abcd1234".to_string());

            let mut buf = gag::BufferRedirect::stdout().expect("should buffer stdout");
            maybe_print_redacted_config(&config);
            let mut output = String::new();
            buf.read_to_string(&mut output)
                .expect("output should be readable");

            assert!(output.is_empty());
        }

        // maybe_print_redacted_config output when verbose
        {
            let mut config = Config::default();
            config.verbose = Some(true);
            config.args.verbose = false;
            config.token = Some("abcd1234".to_string());

            let mut buf = gag::BufferRedirect::stdout().expect("should buffer stdout");
            maybe_print_redacted_config(&config);
            let mut output = String::new();
            buf.read_to_string(&mut output)
                .expect("output should be readable");

            assert!(output.contains("abcdxxxx"));
        }
    }
}
