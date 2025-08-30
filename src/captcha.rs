// src/captcha.rs
use crate::state::{AppState, Pending};
use crate::utils::mention;
use anyhow::Result;
use chrono::{Duration as ChronoDuration, Utc};
use std::sync::Arc;
use teloxide::prelude::*;
use teloxide::types::{InlineKeyboardButton, InlineKeyboardMarkup, MessageId, User};

pub async fn ask_captcha(
    bot: &Bot,
    state: Arc<AppState>,
    chat_id: ChatId,
    user: &User,
) -> Result<()> {
    // 1) Боты: если не в whitelist — баним, если в whitelist — пропускаем
    if user.is_bot {
        if bot
            .get_me()
            .await
            .map(|me| me.id == user.id)
            .unwrap_or(false)
        {
            return Ok(());
        }
        if state.is_bot_allowed(user.id) {
            return Ok(());
        }
        // бан «навсегда» (или как хочешь)
        let _ = bot.ban_chat_member(chat_id, user.id).await;
        return Ok(());
    }

    // 2) Люди — капча
    let user_id = user.id; // объявляем до использования
    let mention_text = mention(bot, chat_id, user_id).await;

    let sent = bot
        .send_message(
            chat_id,
            format!(
                "{mention_text}, нажмите кнопку за {} секунд",
                state.cfg.captcha_timeout_secs
            ),
        )
        .parse_mode(teloxide::types::ParseMode::Html)
        .reply_markup(InlineKeyboardMarkup::new([[
            InlineKeyboardButton::callback("✅ I’m human", format!("ok:{}", user_id.0)),
        ]]))
        .await?;

    state.pending.insert(
        AppState::key(chat_id, user_id),
        Pending {
            _user: user_id.0,
            captcha_msg_id: sent.id.0,
            _deadline: std::time::Instant::now() + state.timeout(),
        },
    );

    let bot_clone = bot.clone();
    let state_clone = state.clone();
    tokio::spawn(async move {
        tokio::time::sleep(state_clone.timeout()).await;

        if let Some((_key, pend)) = state_clone.pending.remove(&AppState::key(chat_id, user_id)) {
            // пример: бан на неделю
            let until = Utc::now() + ChronoDuration::days(7);
            let _ = bot_clone
                .ban_chat_member(chat_id, user_id)
                .until_date(until)
                .await;
            let _ = bot_clone
                .delete_message(chat_id, MessageId(pend.captcha_msg_id))
                .await;
        }
    });

    Ok(())
}

pub async fn on_callback(bot: Bot, state: Arc<AppState>, q: CallbackQuery) -> Result<()> {
    bot.answer_callback_query(q.id).await.ok();

    let msg = match &q.message {
        Some(m) => m,
        None => return Ok(()),
    };
    let chat_id = msg.chat().id;
    let from = &q.from;

    // подтверждать могут только люди; ботов игнорим
    if from.is_bot {
        return Ok(());
    }

    if let Some((_key, pend)) = state.pending.remove(&AppState::key(chat_id, from.id)) {
        let _ = bot
            .delete_message(chat_id, MessageId(pend.captcha_msg_id))
            .await;

        let welcome = mention(&bot, chat_id, from.id).await;
        let _ = bot
            .send_message(chat_id, format!("Добро пожаловать, {welcome}!"))
            .parse_mode(teloxide::types::ParseMode::Html)
            .await;
    }

    Ok(())
}
