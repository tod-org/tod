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
    use crate::time::SystemTimeProvider;
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

    #[test]
    fn fixed_time_provider_returns_expected_values_for_multiple_timezones() {
        let provider = FixedTimeProvider;

        let utc = chrono_tz::UTC;
        let los_angeles = chrono_tz::America::Los_Angeles;

        let utc_time = provider.now(utc);
        let la_time = provider.now(los_angeles);

        assert_eq!(utc_time.to_rfc3339(), "2025-05-10T10:00:00+00:00");
        assert_eq!(la_time.to_rfc3339(), "2025-05-10T03:00:00-07:00");
        assert_eq!(provider.today(utc).to_string(), "2025-05-10");
        assert_eq!(provider.today(los_angeles).to_string(), "2025-05-10");
    }

    #[test]
    fn system_time_provider_handles_utc_and_non_utc_timezones() {
        let provider = SystemTimeProvider;
        let utc = chrono_tz::UTC;
        let los_angeles = chrono_tz::America::Los_Angeles;

        let utc_now = provider.now(utc);
        let la_now = provider.now(los_angeles);

        // Both values should represent the same "current instant", with only a small call-time delta.
        let la_now_as_utc = la_now.with_timezone(&utc);
        assert!((utc_now.timestamp() - la_now_as_utc.timestamp()).abs() <= 1);
        assert_eq!(utc_now.timezone(), utc);
        assert_eq!(la_now.timezone(), los_angeles);
    }
}
