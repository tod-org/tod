// Used for testing where a fixed time is needed for the "now" and "today" functions.
// Returns a fixed time (10:28 AM UTC) for "today", using the system's local date.
// This allos us to have a consistent time for testing, regardless of the system's current time.

#![cfg(test)]

// src/test_time.rs
use crate::time::TimeProvider;
use chrono::{DateTime, NaiveDate, TimeZone};
use chrono_tz::Tz;

/// A fixed time provider for testing purposes.
/// This provider returns a fixed date and time (2025-05-10 10:28:00)
#[derive(Clone, Debug)]
pub struct FixedTimeProvider;

impl TimeProvider for FixedTimeProvider {
    fn now(&self, tz: Tz) -> DateTime<Tz> {
        let dt_utc = chrono::NaiveDate::from_ymd_opt(2025, 5, 10)
            .expect("expected value or result, got None or Err")
            .and_hms_opt(10, 0, 0)
            .expect("expected value or result, got None or Err");
        tz.from_utc_datetime(&dt_utc)
    }

    fn today(&self, tz: Tz) -> NaiveDate {
        self.now(tz).date_naive() // Guarantees alignment
    }
    // Returns a fixed UTC string for testing purposes corresponding to the fixed time
    fn now_string(&self, tz: Tz) -> String {
        self.now(tz).to_rfc3339()
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn fixed_time_provider_returns_fixed_times() {
        let tz = chrono_tz::UTC;
        let provider = FixedTimeProvider;
        let fixed_time = provider.now(tz);
        let fixed_string = provider.now_string(tz);

        // This is what the fixed time is set to in fixed_test_utc_string
        let expected = tz.with_ymd_and_hms(2025, 5, 10, 10, 0, 0).unwrap();
        let expected_string = "2025-05-10T10:00:00+00:00".to_string();

        // Make sure the datetime is exactly as expected
        assert_eq!(fixed_time, expected);
        assert_eq!(fixed_time.date_naive(), expected.date_naive());
        assert_eq!(fixed_string, expected_string);
    }
}
