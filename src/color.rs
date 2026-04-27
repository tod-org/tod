use colored::*;

fn apply_color(str: &str, color: fn(String) -> ColoredString) -> String {
    if cfg!(test) {
        return str.to_string();
    }

    color(str.to_string()).to_string()
}

/// Wraps `str` in green terminal color codes.
pub fn green_string(str: &str) -> String {
    apply_color(str, |s| s.green())
}

/// Wraps `str` in red terminal color codes.
pub fn red_string(str: &str) -> String {
    apply_color(str, |s| s.red())
}

/// Wraps `str` in bright-cyan terminal color codes.
pub fn cyan_string(str: &str) -> String {
    apply_color(str, |s| s.bright_cyan())
}

/// Wraps `str` in purple terminal color codes.
pub fn purple_string(str: &str) -> String {
    apply_color(str, |s| s.purple())
}

/// Wraps `str` in blue terminal color codes.
pub fn blue_string(str: &str) -> String {
    apply_color(str, |s| s.blue())
}

/// Wraps `str` in yellow terminal color codes.
pub fn yellow_string(str: &str) -> String {
    apply_color(str, |s| s.yellow())
}

/// Wraps `str` in bright-blue-on-yellow terminal color codes, used for debug output.
pub fn debug_string(str: &str) -> String {
    apply_color(str, |s| s.bright_blue().on_yellow())
}

/// Returns `str` with default (normal) terminal styling applied.
pub fn normal_string(str: &str) -> String {
    String::from(str).normal().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_blue_string() {
        let str = "TEST";
        let test_string = blue_string("TEST");

        assert_eq!(test_string, str);
    }

    #[test]
    fn test_purple_string() {
        let str = "TEST";
        let test_string = purple_string("TEST");

        assert_eq!(test_string, str);
    }

    #[test]
    fn test_green_string() {
        assert_eq!(green_string("OK"), "OK");
    }

    #[test]
    fn test_red_string() {
        assert_eq!(red_string("ERR"), "ERR");
    }

    #[test]
    fn test_cyan_string() {
        assert_eq!(cyan_string("INFO"), "INFO");
    }

    #[test]
    fn test_yellow_string() {
        assert_eq!(yellow_string("WARN"), "WARN");
    }

    #[test]
    fn test_debug_string() {
        assert_eq!(debug_string("DBG"), "DBG");
    }

    #[test]
    fn test_normal_string() {
        // normal_string does not strip ANSI codes in cfg(test) like the others;
        // it calls .normal().to_string() which may add codes outside test mode,
        // but in test mode we just verify the content is preserved.
        let result = normal_string("plain");
        assert!(result.contains("plain"));
    }
}
