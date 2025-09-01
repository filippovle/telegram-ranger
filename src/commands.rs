// src/commands.rs

//! Команды бота.
//! - Админу показываем подробную шпаргалку с описаниями и примерами.
//! - Обычным пользователям — краткое описание и ссылка на установку/README.
//! В дальнейшем строки легко вынести в i18n.

use crate::state::AppState;
use crate::utils::normalize_username;
use anyhow::Result;
use std::sync::Arc;
use teloxide::prelude::*;

// Ссылка на README проекта (отображается в /help для всех)
const README_URL: &str = "https://github.com/filippovle/telegram-ranger#readme";

/// Точка входа для всех команд (и в личке, и в группах).
pub async fn handle_command(
    bot: &Bot,
    state: Arc<AppState>,
    msg: &Message,
    text: &str,
) -> Result<()> {
    let Some(from) = &msg.from else {
        return Ok(());
    };

    let (cmd, arg) = parse_command(text);
    if from.id == state.cfg.admin_id {
        handle_admin_command(bot, state, msg, cmd, arg).await
    } else {
        handle_user_command(bot, msg, cmd).await
    }
}

/* ========================== Админ ========================== */

async fn handle_admin_command(
    bot: &Bot,
    state: Arc<AppState>,
    msg: &Message,
    cmd: &str,
    arg: Option<&str>,
) -> Result<()> {
    match cmd {
        "start" | "help" => {
            bot.send_message(msg.chat.id, admin_help_text())
                .parse_mode(teloxide::types::ParseMode::Html)
                .await?;
        }

        // ---- БОТЫ (по id или @username) ----
        "allowbot" => {
            let Some(a) = arg else {
                return send_usage(
                    bot,
                    msg,
                    "Usage: <code>/allowbot &lt;id|@username&gt;</code>",
                )
                .await;
            };
            if let Some(id) = parse_numeric(a) {
                state.allow_bot_id(id);
                bot.send_message(msg.chat.id, format!("✅ Bot {id} allowed (id)"))
                    .await?;
            } else {
                state.allow_bot_username(a);
                bot.send_message(msg.chat.id, format!("✅ Bot {a} allowed (username)"))
                    .await?;
            }
        }
        "denybot" => {
            let Some(a) = arg else {
                return send_usage(
                    bot,
                    msg,
                    "Usage: <code>/denybot &lt;id|@username&gt;</code>",
                )
                .await;
            };
            if let Some(id) = parse_numeric(a) {
                state.deny_bot_id(id);
                bot.send_message(msg.chat.id, format!("⛔ Bot {id} denied (id)"))
                    .await?;
            } else {
                state.deny_bot_username(a);
                bot.send_message(msg.chat.id, format!("⛔ Bot {a} denied (username)"))
                    .await?;
            }
        }

        // ---- ЛЮДИ (по id или @username) ----
        "allowuser" => {
            let Some(a) = arg else {
                return send_usage(
                    bot,
                    msg,
                    "Usage: <code>/allowuser &lt;id|@username&gt;</code>",
                )
                .await;
            };
            if let Some(id) = parse_numeric(a) {
                state.allow_user_id(id);
                bot.send_message(msg.chat.id, format!("✅ User {id} allowed"))
                    .await?;
            } else {
                let uname = normalize_username(a);
                if uname.is_empty() {
                    return send_usage(
                        bot,
                        msg,
                        "Usage: <code>/allowuser &lt;id|@username&gt;</code>",
                    )
                    .await;
                }
                state.allow_username(&uname);
                bot.send_message(msg.chat.id, format!("✅ User @{uname} allowed"))
                    .await?;
            }
        }
        "denyuser" => {
            let Some(a) = arg else {
                return send_usage(
                    bot,
                    msg,
                    "Usage: <code>/denyuser &lt;id|@username&gt;</code>",
                )
                .await;
            };
            if let Some(id) = parse_numeric(a) {
                state.deny_user_id(id);
                bot.send_message(msg.chat.id, format!("⛔ User {id} denied"))
                    .await?;
            } else {
                let uname = normalize_username(a);
                if uname.is_empty() {
                    return send_usage(
                        bot,
                        msg,
                        "Usage: <code>/denyuser &lt;id|@username&gt;</code>",
                    )
                    .await;
                }
                state.deny_username(&uname);
                bot.send_message(msg.chat.id, format!("⛔ User @{uname} denied"))
                    .await?;
            }
        }

        "listallow" => {
            let bots: Vec<String> = state
                .bot_whitelist_ids
                .iter()
                .map(|x| x.to_string())
                .collect();
            let bot_names: Vec<String> = state
                .bot_whitelist_names
                .iter()
                .map(|s| format!("@{}", s.clone()))
                .collect();
            let uids: Vec<String> = state
                .user_whitelist_ids
                .iter()
                .map(|x| x.to_string())
                .collect();
            let unames: Vec<String> = state
                .user_whitelist_names
                .iter()
                .map(|s| format!("@{}", s.clone()))
                .collect();

            let msg_text = format!(
                "<b>Whitelists</b>\n\
                 <b>Bots (ids)</b>: {}\n\
                 <b>Bots (names)</b>: {}\n\
                 <b>Users (ids)</b>: {}\n\
                 <b>Users (names)</b>: {}",
                list_or_none(&bots),
                list_or_none(&bot_names),
                list_or_none(&uids),
                list_or_none(&unames),
            );

            bot.send_message(msg.chat.id, msg_text)
                .parse_mode(teloxide::types::ParseMode::Html)
                .await?;
        }

        "about" => {
            bot.send_message(msg.chat.id, about_text())
                .parse_mode(teloxide::types::ParseMode::Html)
                .await?;
        }

        _ => {
            // Нестрогий fallback: подскажем /help
            bot.send_message(
                msg.chat.id,
                "Неизвестная команда. Посмотри <b>/help</b> для списка и примеров.",
            )
            .parse_mode(teloxide::types::ParseMode::Html)
            .await?;
        }
    }
    Ok(())
}

/* ======================== Пользователь ======================== */

async fn handle_user_command(bot: &Bot, msg: &Message, cmd: &str) -> Result<()> {
    match cmd {
        "start" | "help" => {
            bot.send_message(msg.chat.id, user_help_text())
                .parse_mode(teloxide::types::ParseMode::Html)
                .await?;
        }
        "about" => {
            bot.send_message(msg.chat.id, about_text())
                .parse_mode(teloxide::types::ParseMode::Html)
                .await?;
        }
        _ => { /* тихо игнорируем */ }
    }
    Ok(())
}

/* ========================== Тексты ========================== */

fn admin_help_text() -> String {
    r#"<b>Telegram Ranger — справка (админ)</b>

<b>Что делает бот</b>
• Капча для новых участников (кнопка с таймаутом).
• По таймауту — кик/бан (настраивается).
• Whitelist пользователей и ботов (по ID или @username).
• Можно включить удаление сообщений непроверенных пользователей.

<b>Быстрые команды</b>
<pre>/allowbot &lt;id|@username&gt;</pre>
Добавить бота в белый список. Идентификатор — числовой ID или @username (без учёта регистра).

<pre>/denybot &lt;id|@username&gt;</pre>
Убрать бота из белого списка.

<pre>/allowuser &lt;id|@username&gt;</pre>
Добавить человека в белый список (пройдёт без капчи).

<pre>/denyuser &lt;id|@username&gt;</pre>
Убрать человека из белого списка.

<pre>/listallow</pre>
Показать текущие whitelist'ы.

<pre>/about</pre>
Информация о проекте и ссылка на README.

<b>Полезно знать</b>
• @username обрабатывается без учёта регистра и без «@».
• Для кика по таймауту у бота должны быть права администратора на «Удаление участников» и «Ограничение участников».
• Настройки берутся из <code>.env</code>.
"#.to_string()
}

fn user_help_text() -> String {
    format!(
        r#"<b>Telegram Ranger</b>
Я помогаю защищать группы от спама: прошу новичков подтвердить, что они человек.

Если хочешь установить такого же бота к себе — смотри инструкцию:
<a href="{url}">{url}</a>

Доступные команды в этом чате:
• <b>/help</b> — краткая справка.
• Остальные команды доступны администратору группы."#,
        url = README_URL
    )
}

fn about_text() -> String {
    format!(
        r#"<b>Telegram Ranger</b>
Исходники и руководство: <a href="{url}">{url}</a>
Автор: Lev Filippov
Лицензия: MIT"#,
        url = README_URL
    )
}

/* ======================== Утилиты ======================== */

async fn send_usage(bot: &Bot, msg: &Message, usage_html: &str) -> Result<()> {
    bot.send_message(msg.chat.id, usage_html)
        .parse_mode(teloxide::types::ParseMode::Html)
        .await?;
    Ok(())
}

/// Возвращает ("cmd", Some("аргументы")) для строк вида `/cmd@bot arg1 arg2`.
fn parse_command(text: &str) -> (&str, Option<&str>) {
    let t = text.trim();
    if !t.starts_with('/') {
        return ("", None);
    }

    let mut it = t[1..].splitn(2, char::is_whitespace);
    let head = it.next().unwrap_or("");
    let arg = it.next().map(str::trim).filter(|s| !s.is_empty());

    // "/cmd@BotUserName" -> "cmd"
    let cmd = head.split('@').next().unwrap_or(head);
    (cmd, arg)
}

fn parse_numeric(s: &str) -> Option<u64> {
    s.trim().parse::<u64>().ok()
}

fn list_or_none(items: &[String]) -> String {
    if items.is_empty() {
        "(none)".into()
    } else {
        items.join(", ")
    }
}
