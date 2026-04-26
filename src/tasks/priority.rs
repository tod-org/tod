use crate::color;
use std::fmt::Display;

/// Add to all_priorities function if adding another priority
#[derive(serde_repr::Serialize_repr, serde_repr::Deserialize_repr, Debug, Clone, Eq, PartialEq)]
#[repr(u8)]
pub enum Priority {
    None = 1,
    Low = 2,
    Medium = 3,
    High = 4,
}

impl Display for Priority {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Priority::None => write!(f, "{}", color::normal_string("NONE (P4)")),
            Priority::Low => write!(f, "{}", color::blue_string("LOW (P3)")),
            Priority::Medium => write!(f, "{}", color::yellow_string("MEDIUM (P2)")),
            Priority::High => write!(f, "{}", color::red_string("HIGH (P1)")),
        }
    }
}

impl Priority {
    pub fn to_integer(&self) -> u8 {
        match self {
            Priority::None => 1,
            Priority::Low => 2,
            Priority::Medium => 3,
            Priority::High => 4,
        }
    }
}

pub fn from_integer(priority: &Option<u8>) -> Option<Priority> {
    match priority {
        None => None,
        Some(1) => Some(Priority::None),
        Some(2) => Some(Priority::Low),
        Some(3) => Some(Priority::Medium),
        Some(4) => Some(Priority::High),
        Some(_) => unreachable!(),
    }
}

pub fn all_priorities() -> Vec<Priority> {
    vec![
        Priority::None,
        Priority::Low,
        Priority::Medium,
        Priority::High,
    ]
}
#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_all_priorities() {
        let result = all_priorities();
        let expected = vec![
            Priority::None,
            Priority::Low,
            Priority::Medium,
            Priority::High,
        ];

        assert_eq!(result, expected)
    }

    #[test]
    fn test_from_integer() {
        let result = from_integer(&Some(1));
        let expected = Some(Priority::None);

        assert_eq!(result, expected);

        let result = from_integer(&Some(4));
        let expected = Some(Priority::High);

        assert_eq!(result, expected);

        // All variants
        assert_eq!(from_integer(&Some(2)), Some(Priority::Low));
        assert_eq!(from_integer(&Some(3)), Some(Priority::Medium));

        // None input
        assert_eq!(from_integer(&None), None);
    }

    #[test]
    fn test_to_integer() {
        let result = Priority::None.to_integer();
        let expected = 1;

        assert_eq!(result, expected);

        let result = Priority::High.to_integer();
        let expected = 4;

        assert_eq!(result, expected);

        // All variants
        assert_eq!(Priority::Low.to_integer(), 2);
        assert_eq!(Priority::Medium.to_integer(), 3);
    }

    #[test]
    fn test_fmt() {
        let result = Priority::None.to_string();
        let expected = String::from("NONE (P4)");

        assert_eq!(result, expected);

        let result = Priority::High.to_string();
        let expected = String::from("HIGH (P1)");

        assert_eq!(result, expected);

        // Remaining variants
        assert_eq!(Priority::Low.to_string(), "LOW (P3)");
        assert_eq!(Priority::Medium.to_string(), "MEDIUM (P2)");
    }
}
