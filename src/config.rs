// src/config.rs
use teloxide::types::UserId;

#[derive(Clone)]
pub struct Config {
    pub captcha_timeout_secs: u64,
    pub admin_id: UserId,
    pub kick_ban_minutes: i64,
    pub delete_unverified_messages: bool,
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

        let kick_ban_minutes = std::env::var("KICK_BAN_MINUTES")
            .ok()
            .and_then(|s| s.parse::<i64>().ok())
            .unwrap_or(0);

        let delete_unverified_messages = std::env::var("DELETE_UNVERIFIED_MESSAGES")
            .map(|v| matches!(v.to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on"))
            .unwrap_or(false);

        Self {
            captcha_timeout_secs,
            admin_id,
            kick_ban_minutes,
            delete_unverified_messages,
        }
    }
}
