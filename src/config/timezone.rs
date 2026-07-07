use crate::{config::Config, errors::Error, todoist};

impl Config {
    /// Set timezone on Config struct only
    pub fn with_timezone(self: &Config, timezone: &str) -> Config {
        Config {
            timezone: Some(timezone.into()),
            ..self.clone()
        }
    }

    // Get timezone from config, or API if necessary
    pub fn get_timezone(&self) -> Result<String, Error> {
        self.timezone.clone().ok_or_else(|| Error {
            message: "Must set timezone".to_string(),
            source: "get_timezone".to_string(),
        })
    }

    pub async fn maybe_set_timezone(self) -> Result<Config, Error> {
        if self.timezone.is_none() {
            self.set_timezone().await
        } else {
            Ok(self)
        }
    }

    /// Set timezone and save to disk
    pub async fn set_timezone(self) -> Result<Config, Error> {
        let user = todoist::get_user_data(&self).await?;
        let mut config = self.with_timezone(&user.tz_info.timezone);
        config.save().await?;

        Ok(config)
    }
}
