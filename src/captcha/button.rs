use super::*;
use teloxide::types::{InlineKeyboardButton, InlineKeyboardMarkup};

pub struct ButtonCaptcha;

#[async_trait]
impl Captcha for ButtonCaptcha {
    fn mode(&self) -> CaptchaMode {
        CaptchaMode::Button
    }

    async fn ask(
        &self,
        bot: &Bot,
        state: &AppState,
        chat_id: ChatId,
        user: &User,
    ) -> Result<Challenge> {
        let mention_text = mention(bot, chat_id, user.id).await;

        let msg = bot
            .send_message(
                chat_id,
                format!(
                    "{mention_text}, нажмите кнопку за {} секунд",
                    state.cfg.captcha_timeout_secs
                ),
            )
            .parse_mode(ParseMode::Html)
            .reply_markup(InlineKeyboardMarkup::new([[
                InlineKeyboardButton::callback("✅ I’m human", format!("ok:{}", user.id.0)),
            ]]))
            .await?;

        Ok(Challenge {
            message: msg,
            expected_answer: None,
        })
    }

    async fn on_callback(
        &self,
        _bot: &Bot,
        _state: Arc<AppState>,
        q: &CallbackQuery,
    ) -> Result<bool> {
        Ok(q.data.as_deref().map_or(false, |d| d.starts_with("ok:")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn button_mode() {
        let c = ButtonCaptcha;
        assert_eq!(c.mode(), CaptchaMode::Button);
    }
}
