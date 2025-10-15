use colored::*;

fn apply_color(str: &str, color: fn(String) -> ColoredString) -> String {
    if cfg!(test) {
        return str.to_string();
    }

    color(str.to_string()).to_string()
}

pub fn green_string(str: &str) -> String {
    apply_color(str, |s| s.green())
}

pub fn red_string(str: &str) -> String {
    apply_color(str, |s| s.red())
}

pub fn cyan_string(str: &str) -> String {
    apply_color(str, |s| s.bright_cyan())
}

pub fn purple_string(str: &str) -> String {
    apply_color(str, |s| s.purple())
}

pub fn blue_string(str: &str) -> String {
    apply_color(str, |s| s.blue())
}

pub fn yellow_string(str: &str) -> String {
    apply_color(str, |s| s.yellow())
}

pub fn debug_string(str: &str) -> String {
    apply_color(str, |s| s.bright_blue().on_yellow())
}

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
}
