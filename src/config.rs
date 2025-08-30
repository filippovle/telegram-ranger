// src/config.rs
use teloxide::types::UserId;

#[derive(Clone)]
pub struct Config {
    pub captcha_timeout_secs: u64,
    pub admin_id: UserId,
}

impl Config {
    pub fn from_env() -> Self {
        let captcha_timeout_secs = std::env::var("CAPTCHA_TIMEOUT_SEC")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(60);

        let admin_id = std::env::var("ADMIN_USER_ID")
            .ok()
            .and_then(|s| s.parse::<i64>().ok())
            .map(|id| UserId(id as u64))
            .expect("Set ADMIN_USER_ID=<numeric Telegram user id> in .env");

        Self {
            captcha_timeout_secs,
            admin_id,
        }
    }
}
