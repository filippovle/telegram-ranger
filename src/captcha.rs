//! Капча и обработка callback'ов (без бана по таймауту).
//!
//! Правила:
//! - Боты: если не в whitelist — сразу баним; если в whitelist — пропускаем.
//! - Люди: если в whitelist (по id или @username) — пропускаем; иначе показываем капчу.
//! - По таймауту капчи бан НЕ выполняется: удаляем сообщение-капчу и
//!   отправляем короткое уведомление.
use crate::state::{AppState, Pending};
use crate::utils::mention;
use anyhow::Result;
use chrono::{Duration as ChronoDuration, Utc};
use log::{error, info, warn};
use std::sync::Arc;
use teloxide::prelude::*;
use teloxide::types::ChatPermissions;
use teloxide::types::{InlineKeyboardButton, InlineKeyboardMarkup, Message, MessageId, User};

/// Запросить капчу у пользователя (или забанить бот-аккаунт, если он не в whitelist).
pub async fn ask_captcha(
    bot: &Bot,
    state: Arc<AppState>,
    chat_id: ChatId,
    user: &User,
) -> Result<()> {
    // Полный запрет на отправку чего-либо (чтение сообщений остаётся)
    let no_send = ChatPermissions::empty();

    let until = Utc::now() + ChronoDuration::seconds(state.cfg.captcha_timeout_secs as i64);
    let _ = bot
        .restrict_chat_member(chat_id, user.id, no_send)
        .until_date(until)
        .await;

    // 1) Боты: если не в whitelist — баним; если в whitelist — пропускаем.
    if user.is_bot {
        if !state.is_bot_allowed_user(user) {
            warn!(
                "BANNING bot not in whitelist: {} in chat {}",
                user.id.0, chat_id.0
            );
            let _ = bot.ban_chat_member(chat_id, user.id).await;
        }
        return Ok(());
    }

    // 2) Люди из whitelist — пропускаем без капчи.
    if state.is_user_allowed(user) {
        info!("User {} is whitelisted, skipping captcha", user.id.0);
        return Ok(());
    }

    // 3) Капча для остальных (+ дедуп, чтобы не было дублей).
    let key = AppState::key(chat_id, user.id);
    if state.pending.contains_key(&key) {
        return Ok(()); // уже ждём капчу для этого пользователя
    }

    let sent = send_captcha_message(bot, &state, chat_id, user).await?;

    state.pending.insert(
        key,
        Pending {
            _user: user.id.0,
            captcha_msg_id: sent.id.0,
            _deadline: std::time::Instant::now() + state.timeout(),
            user_message_ids: Vec::new(),
        },
    );

    // Таймаут: удаляем капчу и кикаем (реализация внутри schedule_timeout_cleanup).
    schedule_timeout_cleanup(
        bot.clone(),
        state.clone(),
        chat_id,
        user.id,
        state.timeout(),
    );

    Ok(())
}

/// Отправка сообщения с кнопкой капчи.
async fn send_captcha_message(
    bot: &Bot,
    state: &AppState,
    chat_id: ChatId,
    user: &User,
) -> Result<Message> {
    let mention_text = mention(bot, chat_id, user.id).await;

    let msg = bot
        .send_message(
            chat_id,
            format!(
                "{mention_text}, нажмите кнопку за {} секунд",
                state.cfg.captcha_timeout_secs
            ),
        )
        .parse_mode(teloxide::types::ParseMode::Html)
        .reply_markup(InlineKeyboardMarkup::new([[
            InlineKeyboardButton::callback("✅ I’m human", format!("ok:{}", user.id.0)),
        ]]))
        .await?;

    Ok(msg)
}

/// По истечении таймаута удаляем сообщение-капчу и КИКАЕМ пользователя.
/// Реализовано через временный бан: Telegram удалит пользователя из чата сразу.
fn schedule_timeout_cleanup(
    bot: Bot,
    state: Arc<AppState>,
    chat_id: ChatId,
    user_id: UserId,
    timeout: std::time::Duration,
) {
    tokio::spawn(async move {
        tokio::time::sleep(timeout).await;

        if let Some((_key, pend)) = state.pending.remove(&AppState::key(chat_id, user_id)) {
            // 1) удаляем просроченную капчу
            if let Err(e) = bot
                .delete_message(chat_id, MessageId(pend.captcha_msg_id))
                .await
            {
                warn!("Failed to delete captcha message: {e}");
            }

            // 1b) при включённой опции — удаляем все сообщения, которые юзер успел написать
            if state.cfg.delete_unverified_messages {
                for mid in pend.user_message_ids {
                    let _ = bot.delete_message(chat_id, MessageId(mid)).await;
                }
            }

            // 2) кик/бан
            let minutes = state.cfg.kick_ban_minutes;
            if minutes <= 0 {
                // “Мягкий кик”: баним на минуту, затем разбаниваем — это удалит пользователя
                let until = Utc::now() + ChronoDuration::minutes(1);
                match bot
                    .ban_chat_member(chat_id, user_id)
                    .until_date(until)
                    .await
                {
                    Ok(_) => {
                        // сразу снимаем бан, чтобы мог вернуться
                        if let Err(e) = bot.unban_chat_member(chat_id, user_id).await {
                            warn!("Failed to unban (kick flow): {e}");
                        }
                    }
                    Err(e) => {
                        error!("Failed to ban (kick flow): {e}");
                    }
                }
            } else {
                // Жёсткий вариант: баним на указанные минуты
                let until = Utc::now() + ChronoDuration::minutes(minutes);
                if let Err(e) = bot
                    .ban_chat_member(chat_id, user_id)
                    .until_date(until)
                    .await
                {
                    error!("Failed to ban user for {minutes} min: {e}");
                }
            }

            // 3) сообщение в чат (опционально)
            let _ = bot
                .send_message(
                    chat_id,
                    format!(
                        "⏳ Время на подтверждение истекло — участник \
                         <a href=\"tg://user?id={}\">удалён</a>.",
                        user_id.0
                    ),
                )
                .parse_mode(teloxide::types::ParseMode::Html)
                .await;
        }
    });
}

/// Обработчик нажатия на кнопку капчи.
pub async fn on_callback(bot: Bot, state: Arc<AppState>, q: CallbackQuery) -> Result<()> {
    // Убираем «часики»
    bot.answer_callback_query(q.id).await.ok();

    // Сообщение обязательно должно быть (иначе это inline-режим)
    let Some(msg) = &q.message else {
        return Ok(());
    };
    let chat_id = msg.chat().id;
    let from = &q.from;

    // Боты не проходят капчу (страховка)
    if from.is_bot {
        return Ok(());
    }

    if let Some((_key, pend)) = state.pending.remove(&AppState::key(chat_id, from.id)) {
        // Удаляем сообщение-капчу
        let _ = bot
            .delete_message(chat_id, MessageId(pend.captcha_msg_id))
            .await;

        // Разрешаем обычное общение после прохождения капчи
        let allow = ChatPermissions::SEND_MESSAGES
            | ChatPermissions::SEND_MEDIA_MESSAGES   // аудио/доки/фото/видео/voice/video_notes
            | ChatPermissions::SEND_POLLS
            | ChatPermissions::SEND_OTHER_MESSAGES   // стикеры/игры/inline-боты
            | ChatPermissions::ADD_WEB_PAGE_PREVIEWS;

        let _ = bot.restrict_chat_member(chat_id, from.id, allow).await;

        // Приветствие
        let display = from
            .username
            .as_deref()
            .map(|u| format!("@{u}"))
            .unwrap_or_else(|| format!("user {}", from.id.0));

        let _ = bot
            .send_message(
                chat_id,
                format!(
                    "Добро пожаловать, <a href=\"tg://user?id={}\">{}</a>!",
                    from.id.0, display
                ),
            )
            .parse_mode(teloxide::types::ParseMode::Html)
            .await;
    }

    Ok(())
}
