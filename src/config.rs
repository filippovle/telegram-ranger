use std::str::FromStr;
// src/config.rs
use teloxide::types::UserId;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CaptchaMode {
    Off,
    Button,
    Math2,
    Image,
}

impl FromStr for CaptchaMode {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "off" | "none" | "disabled" => Ok(CaptchaMode::Off),
            "button" | "inline" => Ok(CaptchaMode::Button),
            "math2" | "math" => Ok(CaptchaMode::Math2),
            "image" | "img" => Ok(CaptchaMode::Image),
            _ => Ok(CaptchaMode::Button),
        }
    }
}

#[derive(Clone)]
pub struct Config {
    pub captcha_timeout_secs: u64,
    pub admin_id: UserId,
    pub kick_ban_minutes: i64,
    pub delete_unverified_messages: bool,
    pub captcha_mode: CaptchaMode,
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

        let captcha_mode = std::env::var("CAPTCHA_MODE")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(CaptchaMode::Button);

        Self {
            captcha_timeout_secs,
            admin_id,
            kick_ban_minutes,
            delete_unverified_messages,
            captcha_mode,
        }
    }
}
