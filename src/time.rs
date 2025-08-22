use crate::errors::Error;
use crate::{config::Config, regexes};

use chrono::{DateTime, Duration, NaiveDate, NaiveDateTime, Utc};
use chrono_tz::Tz;
use std::str::FromStr;

pub const FORMAT_DATE: &str = "%Y-%m-%d";
const FORMAT_TIME: &str = "%H:%M";
const FORMAT_DATETIME: &str = "%Y-%m-%dT%H:%M:%S";
const FORMAT_DATETIME_ZULU: &str = "%Y-%m-%dT%H:%M:%SZ";
const FORMAT_DATETIME_LONG: &str = "%Y-%m-%dT%H:%M:%S%.fZ";

pub const FORMAT_DATE_AND_TIME: &str = "%Y-%m-%d %H:%M";

#[cfg(test)] //Fixed Time Provider for Testing
use crate::test_time::FixedTimeProvider;

/// Enum for selecting Time Provider
#[derive(Clone, Debug)]
pub enum TimeProviderEnum {
    // Default to System Time Provider
    System(SystemTimeProvider),
    /// Fixed time provider for testing
    #[cfg(test)]
    Fixed(FixedTimeProvider),
}
//Default to SystemTimeProvider for TimeProviderEnum
impl Default for TimeProviderEnum {
    fn default() -> Self {
        TimeProviderEnum::System(SystemTimeProvider)
    }
}

impl TimeProvider for TimeProviderEnum {
    fn now(&self, tz: Tz) -> DateTime<Tz> {
        match self {
            TimeProviderEnum::System(provider) => provider.now(tz),
            #[cfg(test)]
            TimeProviderEnum::Fixed(provider) => provider.now(tz),
        }
    }
    fn today(&self, tz: Tz) -> NaiveDate {
        match self {
            TimeProviderEnum::System(provider) => provider.today(tz),
            #[cfg(test)]
            TimeProviderEnum::Fixed(provider) => provider.today(tz),
        }
    }
}

pub trait TimeProvider: Send + Sync + Clone {
    fn now(&self, tz: Tz) -> DateTime<Tz>;
    fn today(&self, tz: Tz) -> NaiveDate {
        self.now(tz).date_naive()
    }

    #[allow(dead_code)]
    /// Returns a string representation of the current time in the given timezone. If no timezone is given, it defaults to UTC. Currently only used in tests
    fn now_string(&self, tz: Tz) -> String {
        self.now(tz).to_rfc3339()
    }
}

#[derive(Clone, Debug)]
pub struct SystemTimeProvider;

impl TimeProvider for SystemTimeProvider {
    fn now(&self, tz: Tz) -> DateTime<Tz> {
        Utc::now().with_timezone(&tz)
    }
}

// ----------- DATETIME FUNCTIONS ----------

/// Returns the current time in the given timezone
/// If no timezone is given, it defaults to UTC
pub fn datetime_now(config: &Config) -> Result<DateTime<Tz>, Error> {
    let timezone = config.get_timezone()?;
    let tz = timezone_from_str(&timezone)?;

    Ok(config.time_provider.now(tz))
}

// Checks if datetime is today
pub fn datetime_is_today(datetime: DateTime<Tz>, config: &Config) -> Result<bool, Error> {
    is_date_today(datetime.date_naive(), config)
}

/// Parse DateTime
pub fn datetime_from_str(str: &str, timezone: Tz) -> Result<DateTime<Tz>, Error> {
    match str.len() {
        19 => parse_datetime(str, timezone, FORMAT_DATETIME),
        20 => parse_datetime(str, Tz::UTC, FORMAT_DATETIME_ZULU),
        27 => parse_datetime(str, Tz::UTC, FORMAT_DATETIME_LONG),
        length => Err(Error {
            source: "datetime_from_str".to_string(),
            message: format!("cannot parse {length} length DateTime: {str}"),
        }),
    }
}

fn parse_datetime(str: &str, timezone: Tz, format: &str) -> Result<DateTime<Tz>, Error> {
    let naive_datetime = NaiveDateTime::parse_from_str(str, format)?;
    naive_datetime_to_datetime(naive_datetime, timezone)
}

fn naive_datetime_to_datetime(
    datetime: NaiveDateTime,
    timezone: Tz,
) -> Result<DateTime<Tz>, Error> {
    datetime
        .and_local_timezone(timezone)
        .single()
        .ok_or_else(|| {
            Error::new(
                "naive_datetime_to_datetime",
                "Ambiguous or invalid datetime",
            )
        })
}

/// Checks if string is a datetime in format YYYY-MM-DD HH:MM
pub fn is_datetime(string: &str) -> bool {
    regexes::DATETIME_REGEX.is_match(string)
}

// ----------- DATE FUNCTIONS --------------

/// Parses a date string into a `NaiveDate` - The string can be in the format YYYY-MM-DD or YYYY-MM-DD HH:MM or YYYY-MM-DDTHH:MM:SS or YYYY-MM-DDTHH:MM:SSZ. Timezone is used to convert the date to UTC. If the string is not in one of these formats, an error is returned.
pub fn date_from_str(str: &str, timezone: Tz) -> Result<NaiveDate, Error> {
    let date = match str.len() {
        10 => NaiveDate::parse_from_str(str, FORMAT_DATE)?,
        19 => {
            let naive_datetime = NaiveDateTime::parse_from_str(str, FORMAT_DATETIME)?;
            naive_datetime_to_datetime(naive_datetime, timezone)?.date_naive()
        }
        20 => {
            let naive_datetime = NaiveDateTime::parse_from_str(str, FORMAT_DATETIME_ZULU)?;
            naive_datetime_to_datetime(naive_datetime, timezone)?.date_naive()
        }
        _ => {
            return Err(Error::new(
                "date_from_str",
                &format!("Cannot parse NaiveDate, unknown length: {str}"),
            ));
        }
    };

    Ok(date)
}

/// Checks if string is a date in format YYYY-MM-DD
pub fn is_date(string: &str) -> bool {
    regexes::DATE_REGEX.is_match(string)
}
/// Return today's date in Utc from the config timezone (defaults to UTC)
/// This is used for the "today" command
/// and for the "due" command to check if a date is today
pub fn naive_date_today(config: &Config) -> Result<NaiveDate, Error> {
    let tz = timezone_from_str(&config.get_timezone()?)?;
    Ok(config.time_provider.today(tz))
}

/// Returns today's date in given timezone for testing. Only used in tests currently but included for completeness.
#[allow(dead_code)]
pub fn naive_date_today_from_tz(tz: Tz) -> Result<NaiveDate, Error> {
    Ok(chrono::Utc::now().with_timezone(&tz).date_naive())
}

// Check if date is today
pub fn is_date_today(date: NaiveDate, config: &Config) -> Result<bool, Error> {
    let date_string = date.format(FORMAT_DATE).to_string();
    let today_string = date_string_today(config)?;
    Ok(date_string == today_string)
}

// Converts a date string to a NaiveDate
pub fn date_string_to_naive_date(date_string: &str) -> Result<NaiveDate, Error> {
    NaiveDate::from_str(date_string).map_err(Error::from)
}

// / Check if date is in the past
pub fn is_date_in_past(date: NaiveDate, config: &Config) -> Result<bool, Error> {
    Ok(naive_date_days_in_future(date, config)? < 0)
}

/// Returns 0 if today, negative if date given is in the past
pub fn naive_date_days_in_future(date: NaiveDate, config: &Config) -> Result<i64, Error> {
    let duration: Duration = date - naive_date_today(config)?;
    Ok(duration.num_days())
}
// ----------- STRING FUNCTIONS --------------

/// Return today's date in format 2021-09-16
pub fn date_string_today(config: &Config) -> Result<String, Error> {
    let timezone = config.get_timezone()?;
    let tz = timezone_from_str(&timezone)?;
    let today = config.time_provider.today(tz);

    Ok(today.format(FORMAT_DATE).to_string())
}

// Formats a date to a string
pub fn date_to_string(date: &NaiveDate, config: &Config) -> Result<String, Error> {
    if is_date_today(*date, config)? {
        Ok("Today".into())
    } else {
        Ok(date.format(FORMAT_DATE).to_string())
    }
}

// Formats a datetime to a string
pub fn datetime_to_string(datetime: &DateTime<Tz>, config: &Config) -> Result<String, Error> {
    let timezone = config.get_timezone()?;
    let tz = timezone_from_str(&timezone)?;
    if datetime_is_today(*datetime, config)? {
        Ok(datetime.with_timezone(&tz).format(FORMAT_TIME).to_string())
    } else {
        Ok(datetime.with_timezone(&tz).to_string())
    }
}

// ----------- TZ FUNCTIONS --------------

pub fn timezone_from_str(timezone_string: &str) -> Result<Tz, Error> {
    timezone_string
        .parse::<Tz>()
        .or_else(|_| parse_gmt_to_timezone(timezone_string))
}

/// For when we get offsets like GMT -7:00
fn parse_gmt_to_timezone(gmt: &str) -> Result<Tz, Error> {
    let split: Vec<&str> = gmt.split_whitespace().collect();
    let offset = split
        .get(1)
        .ok_or_else(|| Error::new("parse_timezone", "Invalid GMT format: missing offset"))?;
    let offset = offset.replace(":00", "");
    let offset = offset.replace(':', "");
    let offset_num = offset.parse::<i32>()?;

    let tz_string = format!(
        "Etc/GMT{}",
        if offset_num < 0 {
            "+".to_string()
        } else {
            "-".to_string()
        } + &offset_num.abs().to_string()
    );
    tz_string.parse().map_err(Error::from)
}
// -----------------------------------------

#[cfg(test)]
mod tests {
    use crate::{config, time};
    use pretty_assertions::assert_eq;

    use super::*;
    use chrono_tz::Tz;

    #[test]
    fn test_is_date() {
        assert!(is_date("2022-10-05"));
        assert!(!is_date("22-10-05"));
        assert!(!is_date("2022-10-05 24:02"));
        assert!(!is_date("today"));
    }

    #[test]
    fn test_is_datetime() {
        assert!(!is_datetime("2022-10-05"));
        assert!(!is_datetime("22-10-05"));
        assert!(is_datetime("2022-10-05 24:02"));
        assert!(!is_datetime("today"));
    }

    #[test]
    fn test_timezone_from_string() {
        assert_eq!(
            timezone_from_str("America/Los_Angeles"),
            Ok(Tz::America__Los_Angeles),
        );

        assert_eq!(timezone_from_str("GMT -7:00"), Ok(Tz::Etc__GMTPlus7),);
    }

    #[test]
    fn test_today_date_from_tz_utc() {
        let tz = Tz::UTC;
        let result =
            naive_date_today_from_tz(tz).expect("failed to compute today's date for timezone");
        let expected = chrono::Utc::now().with_timezone(&tz).date_naive();
        assert_eq!(result, expected);
    }

    #[test]
    fn test_today_date_from_tz_pacific() {
        let tz = Tz::America__Los_Angeles;
        let result =
            naive_date_today_from_tz(tz).expect("failed to compute today's date for timezone");
        let expected = chrono::Utc::now().with_timezone(&tz).date_naive();
        assert_eq!(result, expected);
    }

    #[test]
    fn trait_default_today_is_used() {
        let provider = SystemTimeProvider;
        let tz: Tz = "UTC".parse().expect("failed to parse timezone 'UTC'");
        let expected = provider.now(tz).date_naive();
        let today = provider.today(tz);
        assert_eq!(today, expected);
    }

    #[tokio::test]
    async fn errors_when_no_timezone() {
        let path = config::generate_path()
            .await
            .expect("failed to generate config path");
        let config = Config::new(None, path)
            .await
            .expect("failed to create Config with path");
        assert_matches!(config.get_timezone(), Err(Error { .. }));
    }

    #[test]
    fn fallback_to_utc_now_when_today_date_from_tz_fails() {
        let tz: Tz = chrono_tz::UTC;

        let result = time::naive_date_today_from_tz(tz)
            .unwrap_or_else(|_| Utc::now().with_timezone(&tz).date_naive());

        let expected = Utc::now().with_timezone(&tz).date_naive();

        // Allow for edge-of-day differences
        assert!(
            result == expected || (result - expected).num_days().abs() <= 1,
            "Got {result}, expected ~{expected}"
        );
    }
    #[tokio::test]
    async fn test_default_config_uses_system_time_provider() {
        // Create a default config
        let config = Config::default();

        // Parse a timezone (e.g., UTC)
        let tz: Tz = "UTC".parse().expect("failed to parse timezone 'UTC'");

        // Call the `today` method via the `time_provider`
        let today_from_provider = config.time_provider.today(tz);

        // Get today's date directly from `SystemTimeProvider` for comparison
        let system_provider = SystemTimeProvider;
        let today_from_system = system_provider.today(tz);

        // Assert that the `time_provider` in the default config behaves like `SystemTimeProvider`
        assert_eq!(today_from_provider, today_from_system)
    }
}
