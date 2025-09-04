//! Капчи-стратегии + общий роутинг. Публичный API сохранён:
//! - ask_captcha(bot, state, chat_id, user)
//! - on_callback(bot, state, q)
//! Добавлено:
//! - on_user_message(bot, state, &msg) — нужно дергать из message-хэндлера,
//!   чтобы math2 могла принять ответ текстом.

mod button;
mod math2;
// mod image;

fn fmt_until_date(u: &teloxide::types::UntilDate) -> String {
    use teloxide::types::UntilDate as UD;
    match u {
        UD::Date(dt) => dt.to_rfc3339(),
        #[allow(unreachable_patterns)]
        other => format!("{other:?}"),
    }
}

use crate::config::CaptchaMode;
use crate::state::{AppState, Pending};
use crate::utils::mention;
use anyhow::Result;
use async_trait::async_trait;
use chrono::{Duration as ChronoDuration, Utc};
use log::{debug, error, warn};
// <— info убран
use std::collections::HashSet;
use std::sync::Arc;
use teloxide::prelude::*;
use teloxide::types::{CallbackQuery, ChatPermissions, Message, MessageId, ParseMode, User};

pub use button::ButtonCaptcha;
pub use math2::Math2Captcha;

#[derive(Debug)]
pub struct Challenge {
    pub message: Message,
    pub expected_answer: Option<String>,
}

#[async_trait]
pub trait Captcha: Send + Sync {
    fn mode(&self) -> CaptchaMode;

    async fn ask(
        &self,
        bot: &Bot,
        state: &AppState,
        chat_id: ChatId,
        user: &User,
    ) -> Result<Challenge>;

    async fn on_callback(
        &self,
        _bot: &Bot,
        _state: Arc<AppState>,
        _q: &CallbackQuery,
    ) -> Result<bool> {
        Ok(false)
    }

    async fn on_text(&self, _bot: &Bot, _state: Arc<AppState>, _msg: &Message) -> Result<bool> {
        Ok(false)
    }
}

fn provider(mode: CaptchaMode) -> Option<Box<dyn Captcha>> {
    match mode {
        CaptchaMode::Off => None,
        CaptchaMode::Button => Some(Box::new(ButtonCaptcha)),
        CaptchaMode::Math2 => Some(Box::new(Math2Captcha)),
        CaptchaMode::Image => {
            // TODO: ImageCaptcha
            Some(Box::new(ButtonCaptcha))
        }
    }
}

/// PUBLIC API (совместим с прежним): запросить капчу / пропустить.
pub async fn ask_captcha(
    bot: &Bot,
    state: Arc<AppState>,
    chat_id: ChatId,
    user: &User,
) -> Result<()> {
    // Полностью запретить сообщения на время проверки (кроме math2 — ей нужен текст)
    let until = Utc::now() + ChronoDuration::seconds(state.cfg.captcha_timeout_secs as i64);
    if !matches!(state.cfg.captcha_mode, CaptchaMode::Math2) {
        let no_send = ChatPermissions::empty();
        let _ = bot
            .restrict_chat_member(chat_id, user.id, no_send)
            .until_date(until)
            .await;
    }

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

    // 2) Люди из whitelist — пропускаем без капчи (и снимаем ограничения).
    if state.is_user_allowed(user) || matches!(state.cfg.captcha_mode, CaptchaMode::Off) {
        debug!(
            "Skip captcha: whitelisted or captcha off (user={})",
            user.id.0
        );
        allow_user(bot, chat_id, user.id).await?;
        return Ok(());
    }

    // 3) По режиму .env
    let Some(strategy) = provider(state.cfg.captcha_mode) else {
        allow_user(bot, chat_id, user.id).await?;
        return Ok(());
    };

    // Дедуп (первичная проверка)
    let key = AppState::key(chat_id, user.id);
    if state.pending.contains_key(&key) {
        return Ok(());
    }

    // Показать капчу
    let challenge = strategy.ask(bot, &state, chat_id, user).await?;

    // Считаем таймер один раз
    let timeout = state.timeout();
    let deadline = tokio::time::Instant::now() + timeout;

    // Сохранить Pending (защита от гонки)
    let inserted = state.pending.insert(
        key,
        Pending {
            _user: user.id.0,
            captcha_msg_id: challenge.message.id.0,
            _deadline: std::time::Instant::now() + timeout,
            user_message_ids: Vec::new(),
            captcha_mode: strategy.mode(),
            expected_answer: challenge.expected_answer,
        },
    );

    if inserted.is_some() {
        let _ = bot
            .delete_message(chat_id, MessageId(challenge.message.id.0))
            .await;
        return Ok(());
    }

    // Таймаут по дедлайну
    schedule_timeout_cleanup(bot.clone(), state.clone(), chat_id, user.id, deadline);

    Ok(())
}

/// PUBLIC API (совместим с прежним): обработчик callback'ов.
pub async fn on_callback(bot: Bot, state: Arc<AppState>, q: CallbackQuery) -> Result<()> {
    let qid = q.id.clone();
    bot.answer_callback_query(qid).await.ok();

    let Some(msg) = &q.message else {
        return Ok(());
    };
    let chat_id = msg.chat().id;

    let from = &q.from;
    if from.is_bot {
        return Ok(());
    }

    let key = AppState::key(chat_id, from.id);
    let Some((_k, pend)) = state.pending.get(&key).map(|r| (key, r.clone())) else {
        return Ok(());
    };

    if let Some(strategy) = provider(pend.captcha_mode) {
        let ok = strategy.on_callback(&bot, state.clone(), &q).await?;
        if ok {
            // удалить pending и финализировать
            let (_k, pend) = state.pending.remove(&key).expect("pending exists");
            complete_and_greet(&bot, state, chat_id, from, pend).await?;
        }
    }
    Ok(())
}

/// Обработка текстов пользователя для стратегий типа math2.
/// Вызвать в message-хэндлере ДО основной логики.
pub async fn on_user_message(bot: Bot, state: Arc<AppState>, msg: &Message) -> Result<()> {
    let Some(from) = msg.from() else {
        return Ok(());
    };
    if from.is_bot {
        return Ok(());
    }
    let chat_id = msg.chat.id;

    let key = AppState::key(chat_id, from.id);
    let Some((_k, pend)) = state.pending.get(&key).map(|r| (key, r.clone())) else {
        return Ok(());
    };

    if let Some(strategy) = provider(pend.captcha_mode) {
        let ok = strategy.on_text(&bot, state.clone(), msg).await?;
        if ok {
            let (_k, pend) = state
                .pending
                .remove(&AppState::key(chat_id, from.id))
                .expect("pending exists");
            complete_and_greet(&bot, state, chat_id, from, pend).await?;
        }
    }
    Ok(())
}

// --- общие утилиты для всех стратегий ---

fn schedule_timeout_cleanup(
    bot: Bot,
    state: Arc<AppState>,
    chat_id: ChatId,
    user_id: UserId,
    deadline: tokio::time::Instant,
) {
    // тех.лог — только debug
    let until_in = deadline
        .checked_duration_since(tokio::time::Instant::now())
        .map(|d| d.as_secs())
        .unwrap_or(0);
    debug!(
        "CAPTCHA timeout scheduled in {}s (chat={}, user={})",
        until_in, chat_id.0, user_id.0
    );

    tokio::spawn(async move {
        tokio::time::sleep_until(deadline).await;
        debug!(
            "CAPTCHA timeout fired (chat={}, user={})",
            chat_id.0, user_id.0
        );

        let key = AppState::key(chat_id, user_id);
        let Some((_key, pend)) = state.pending.remove(&key) else {
            debug!(
                "No pending found on timeout (likely solved/removed earlier) (chat={}, user={})",
                chat_id.0, user_id.0
            );
            return;
        };

        // 1) удаляем сообщение-капчу
        match bot
            .delete_message(chat_id, MessageId(pend.captcha_msg_id))
            .await
        {
            Ok(_) => debug!(
                "Deleted captcha message id={} (chat={}, user={})",
                pend.captcha_msg_id, chat_id.0, user_id.0
            ),
            Err(e) => warn!(
                "Failed to delete captcha message id={} (chat={}, user={}): {}",
                pend.captcha_msg_id, chat_id.0, user_id.0, e
            ),
        }

        // 1b) удалить все сообщения пользователя (если включено)
        if state.cfg.delete_unverified_messages {
            let mut ok_cnt = 0usize;
            let mut err_cnt = 0usize;
            for mid in pend.user_message_ids {
                match bot.delete_message(chat_id, MessageId(mid)).await {
                    Ok(_) => ok_cnt += 1,
                    Err(_) => err_cnt += 1,
                }
            }
            debug!(
                "Deleted user messages on timeout: ok={}, err={} (chat={}, user={})",
                ok_cnt, err_cnt, chat_id.0, user_id.0
            );
        }

        // 2) кик/бан
        let minutes = state.cfg.kick_ban_minutes;
        debug!(
            "Applying timeout action: minutes={} (chat={}, user={})",
            minutes, chat_id.0, user_id.0
        );

        if minutes <= 0 {
            // “мягкий кик”
            let until = Utc::now() + ChronoDuration::minutes(1);
            match bot
                .ban_chat_member(chat_id, user_id)
                .until_date(until)
                .await
            {
                Ok(_) => {
                    debug!(
                        "Soft kick applied (ban+unban) (chat={}, user={})",
                        chat_id.0, user_id.0
                    );
                    if let Err(e) = bot.unban_chat_member(chat_id, user_id).await {
                        warn!(
                            "Soft kick unban failed (chat={}, user={}): {}",
                            chat_id.0, user_id.0, e
                        );
                    }
                }
                Err(e) => {
                    error!(
                        "Soft kick ban failed (chat={}, user={}): {}",
                        chat_id.0, user_id.0, e
                    );
                }
            }
        } else {
            // Жёсткий вариант: бан на N минут
            let until = Utc::now() + ChronoDuration::minutes(minutes);
            match bot
                .ban_chat_member(chat_id, user_id)
                .until_date(until)
                .await
            {
                Ok(_) => {
                    debug!(
                        "Temp BAN OK for {} minutes (until {}), chat={}, user={}",
                        minutes,
                        until.to_rfc3339(),
                        chat_id.0,
                        user_id.0
                    );

                    // Контрольный статус после бана
                    match bot.get_chat_member(chat_id, user_id).await {
                        Ok(cm) => {
                            use teloxide::types::ChatMemberKind as CMK;
                            match cm.kind {
                                CMK::Owner(_) => {
                                    warn!("POST-BAN STATUS: OWNER (cannot be banned), chat={}, user={}", chat_id.0, user_id.0);
                                }
                                CMK::Administrator(_) => {
                                    warn!("POST-BAN STATUS: ADMIN (cannot be banned), chat={}, user={}", chat_id.0, user_id.0);
                                }
                                CMK::Member(_) => {
                                    warn!("POST-BAN STATUS: still MEMBER — ban didn't stick, chat={}, user={}", chat_id.0, user_id.0);
                                }
                                CMK::Left => {
                                    debug!(
                                        "POST-BAN STATUS: LEFT (user is out), chat={}, user={}",
                                        chat_id.0, user_id.0
                                    );
                                }
                                CMK::Restricted(r) => {
                                    debug!(
                                        "POST-BAN STATUS: RESTRICTED until {}, chat={}, user={}",
                                        fmt_until_date(&r.until_date),
                                        chat_id.0,
                                        user_id.0
                                    );
                                }
                                CMK::Banned(b) => {
                                    debug!(
                                        "POST-BAN STATUS: BANNED until {}, chat={}, user={}",
                                        fmt_until_date(&b.until_date),
                                        chat_id.0,
                                        user_id.0
                                    );
                                }
                                #[allow(unreachable_patterns)]
                                other => {
                                    debug!(
                                        "POST-BAN STATUS: {:?}, chat={}, user={}",
                                        other, chat_id.0, user_id.0
                                    );
                                }
                            }
                        }
                        Err(e) => {
                            warn!(
                                "get_chat_member after ban failed (chat={}, user={}): {}",
                                chat_id.0, user_id.0, e
                            );
                        }
                    }
                }
                Err(e) => {
                    error!(
                        "Temp BAN failed ({} min) (chat={}, user={}): {}",
                        minutes, chat_id.0, user_id.0, e
                    );
                }
            }
        }

        // 3) сервисное уведомление в чат
        match bot
            .send_message(
                chat_id,
                format!(
                    "⏳ Время на подтверждение истекло — участник <a href=\"tg://user?id={}\">удалён</a>.",
                    user_id.0
                ),
            )
            .parse_mode(ParseMode::Html)
            .await
        {
            Ok(_) => debug!("Posted timeout notice (chat={}, user={})", chat_id.0, user_id.0),
            Err(e) => warn!("Failed to post timeout notice (chat={}, user={}): {}", chat_id.0, user_id.0, e),
        }
    });
}

async fn allow_user(bot: &Bot, chat_id: ChatId, user_id: UserId) -> Result<()> {
    let allow = ChatPermissions::SEND_MESSAGES
        | ChatPermissions::SEND_MEDIA_MESSAGES
        | ChatPermissions::SEND_POLLS
        | ChatPermissions::SEND_OTHER_MESSAGES
        | ChatPermissions::ADD_WEB_PAGE_PREVIEWS;
    let _ = bot.restrict_chat_member(chat_id, user_id, allow).await;
    Ok(())
}

async fn complete_and_greet(
    bot: &Bot,
    _state: Arc<AppState>,
    chat_id: ChatId,
    user: &User,
    pend: Pending,
) -> Result<()> {
    // 1) удалить сообщение-капчу
    let _ = bot
        .delete_message(chat_id, MessageId(pend.captcha_msg_id))
        .await;

    // 2) удалить все сообщения пользователя, накопленные во время ожидания
    if !pend.user_message_ids.is_empty() {
        let mut seen = HashSet::new();
        for mid in pend
            .user_message_ids
            .into_iter()
            .filter(|m| *m > pend.captcha_msg_id)
        {
            if seen.insert(mid) {
                let _ = bot.delete_message(chat_id, MessageId(mid)).await;
            }
        }
    }

    allow_user(bot, chat_id, user.id).await?;

    let display = user
        .username
        .as_deref()
        .map(|u| format!("@{u}"))
        .unwrap_or_else(|| format!("user {}", user.id.0));

    let _ = bot
        .send_message(
            chat_id,
            format!(
                "Добро пожаловать, <a href=\"tg://user?id={}\">{}</a>!",
                user.id.0, display
            ),
        )
        .parse_mode(ParseMode::Html)
        .await;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use std::str::FromStr;
    use teloxide::types::UntilDate;

    #[test]
    fn captcha_mode_parse_variants() {
        assert_eq!(
            CaptchaMode::from_str("button").unwrap(),
            CaptchaMode::Button
        );
        assert_eq!(CaptchaMode::from_str("MATH2").unwrap(), CaptchaMode::Math2);
        assert_eq!(CaptchaMode::from_str("off").unwrap(), CaptchaMode::Off);
        assert_eq!(CaptchaMode::from_str("image").unwrap(), CaptchaMode::Image);
        assert_eq!(CaptchaMode::from_str("???").unwrap(), CaptchaMode::Button); // default
    }

    #[test]
    fn provider_matches_mode() {
        assert!(provider(CaptchaMode::Off).is_none());
        assert!(provider(CaptchaMode::Button).is_some());
        assert!(provider(CaptchaMode::Math2).is_some());
        assert!(provider(CaptchaMode::Image).is_some()); // пока Image -> Button
    }

    #[test]
    fn fmt_until_date_handles_date() {
        let u = UntilDate::Date(Utc::now());
        let s = fmt_until_date(&u);
        assert!(s.contains('T'));
    }
}
