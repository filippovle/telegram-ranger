//! Команды бота. Вход — общий `handle_command`, который сам решает, админ это
//! или нет, и роутит в нужный обработчик.

use crate::state::AppState;
use anyhow::Result;
use std::sync::Arc;
use teloxide::prelude::*;

/// Точка входа для всех команд.
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

/// Команды для админа.
async fn handle_admin_command(
    bot: &Bot,
    state: Arc<AppState>,
    msg: &Message,
    cmd: &str,
    arg: Option<&str>,
) -> Result<()> {
    match cmd {
        "help" | "start" => {
            bot.send_message(
                msg.chat.id,
                "Команды админа:
- /allowbot <id|@username>
- /denybot <id|@username>
- /allowuser <id|@username>
- /denyuser <id|@username>
- /listallow",
            )
            .await?;
        }

        // ---- БОТЫ (по id или @username) ----
        "allowbot" => {
            let Some(a) = arg else {
                bot.send_message(msg.chat.id, "Usage: /allowbot <id|@username>")
                    .await?;
                return Ok(());
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
                bot.send_message(msg.chat.id, "Usage: /denybot <id|@username>")
                    .await?;
                return Ok(());
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
                bot.send_message(msg.chat.id, "Usage: /allowuser <id|@username>")
                    .await?;
                return Ok(());
            };
            if let Some(id) = parse_numeric(a) {
                state.allow_user_id(id);
                bot.send_message(msg.chat.id, format!("✅ User {id} allowed"))
                    .await?;
            } else {
                let uname = normalize_username(a);
                if uname.is_empty() {
                    bot.send_message(msg.chat.id, "Usage: /allowuser <id|@username>")
                        .await?;
                } else {
                    state.allow_username(&uname);
                    bot.send_message(msg.chat.id, format!("✅ User @{uname} allowed"))
                        .await?;
                }
            }
        }
        "denyuser" => {
            let Some(a) = arg else {
                bot.send_message(msg.chat.id, "Usage: /denyuser <id|@username>")
                    .await?;
                return Ok(());
            };
            if let Some(id) = parse_numeric(a) {
                state.deny_user_id(id);
                bot.send_message(msg.chat.id, format!("⛔ User {id} denied"))
                    .await?;
            } else {
                let uname = normalize_username(a);
                if uname.is_empty() {
                    bot.send_message(msg.chat.id, "Usage: /denyuser <id|@username>")
                        .await?;
                } else {
                    state.deny_username(&uname);
                    bot.send_message(msg.chat.id, format!("⛔ User @{uname} denied"))
                        .await?;
                }
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
                "<b>Whitelists</b>\nBots (ids): {}\nBots (names): {}\nUsers (ids): {}\nUsers (names): {}",
                if bots.is_empty() { "(none)".into() } else { bots.join(", ") },
                if bot_names.is_empty() { "(none)".into() } else { bot_names.join(", ") },
                if uids.is_empty() { "(none)".into() } else { uids.join(", ") },
                if unames.is_empty() { "(none)".into() } else { unames.join(", ") },
            );
            bot.send_message(msg.chat.id, msg_text)
                .parse_mode(teloxide::types::ParseMode::Html)
                .await?;
        }

        _ => {}
    }
    Ok(())
}

/// Ответ обычному пользователю (не администратору).
async fn handle_user_command(bot: &Bot, msg: &Message, cmd: &str) -> Result<()> {
    match cmd {
        "start" => {
            bot.send_message(
                msg.chat.id,
                "Привет! Я Telegram Ranger 👋\nДобавь меня в группу — включу капчу для новых участников.",
            )
                .await?;
        }
        "help" => {
            bot.send_message(msg.chat.id, "Команды доступны только администратору.")
                .await?;
        }
        _ => { /* тихо игнорируем */ }
    }
    Ok(())
}

/* -------------------- утилиты парсинга -------------------- */

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

fn normalize_username<S: AsRef<str>>(name: S) -> String {
    let n = name.as_ref().trim();
    let n = n.strip_prefix('@').unwrap_or(n);
    n.to_lowercase()
}
