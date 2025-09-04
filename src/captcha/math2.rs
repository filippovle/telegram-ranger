// math2.rs
use super::*;
use rand::{rng, Rng};
use teloxide::types::ParseMode;

pub struct Math2Captcha;

#[async_trait]
impl Captcha for Math2Captcha {
    fn mode(&self) -> CaptchaMode {
        CaptchaMode::Math2
    }

    async fn ask(
        &self,
        bot: &Bot,
        state: &AppState,
        chat_id: ChatId,
        user: &User,
    ) -> Result<Challenge> {
        let (a, b) = {
            let mut rng = rng();
            (rng.random_range(1..=9), rng.random_range(1..=9))
        };
        let expected = (a as u16 + b as u16).to_string();

        let m = mention(bot, chat_id, user.id).await;

        let msg = bot
            .send_message(
                chat_id,
                format!("{m}, докажите, что вы человек: сколько будет {a} + {b}? Напишите только число за {} секунд.",
                        state.cfg.captcha_timeout_secs),
            )
            .parse_mode(ParseMode::Html)
            .await?;

        Ok(Challenge {
            message: msg,
            expected_answer: Some(expected),
        })
    }

    async fn on_text(&self, _bot: &Bot, state: Arc<AppState>, msg: &Message) -> Result<bool> {
        let Some(from) = msg.from() else {
            return Ok(false);
        };
        let chat_id = msg.chat.id;
        let key = AppState::key(chat_id, from.id);
        let Some((_k, pend)) = state.pending.get(&key).map(|r| (key, r.clone())) else {
            return Ok(false);
        };

        if let (Some(ans), Some(txt)) = (pend.expected_answer.as_ref(), msg.text()) {
            Ok(is_numeric_equal(txt, ans))
        } else {
            Ok(false)
        }
    }
}

/// Чистая функция для юнит-тестов и переиспользования.
fn is_numeric_equal(candidate: &str, expected: &str) -> bool {
    let c = candidate.trim();
    if !c.chars().all(|ch| ch.is_ascii_digit()) {
        return false;
    }
    c == expected
}

#[cfg(test)]
mod tests {
    use super::is_numeric_equal;

    #[test]
    fn numeric_compare_basic() {
        assert!(is_numeric_equal("  7 ", "7"));
        assert!(is_numeric_equal("12", "12"));
        assert!(!is_numeric_equal("07", "7"));
        assert!(!is_numeric_equal("7 ", "8"));
        assert!(!is_numeric_equal("7a", "7"));
    }
}
