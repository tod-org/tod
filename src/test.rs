#[cfg(test)]
pub mod fixtures;
#[cfg(test)]
pub mod responses;

#[cfg(test)]
async fn today_date() -> String {
    let config = fixtures::config().await.with_timezone("America/Vancouver");
    crate::time::date_string_today(&config).expect("expected value or result, got None or Err")
}
