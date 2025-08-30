use crate::state::AppState;
use anyhow::Result;
use std::sync::Arc;
use teloxide::prelude::*;
use teloxide::types::Message;

fn is_admin(state: &AppState, msg: &Message) -> bool {
    msg.from()
        .map(|u| u.id == state.cfg.admin_id)
        .unwrap_or(false)
}

fn parse_command(text: &str) -> (&str, Option<&str>) {
    let mut parts = text.trim().split_whitespace();
    let cmd_raw = parts.next().unwrap_or("");
    // /cmd or /cmd@BotName
    let cmd = cmd_raw
        .trim_start_matches('/')
        .split('@')
        .next()
        .unwrap_or("");
    (cmd, parts.next())
}

pub async fn handle_command(
    bot: &Bot,
    state: Arc<AppState>,
    msg: &Message,
    text: &str,
) -> Result<()> {
    if is_admin(&state, msg) {
        handle_admin_command(bot, state, msg, text).await
    } else {
        handle_user_command(bot, state, msg, text).await
    }
}

pub async fn handle_user_command(
    bot: &Bot,
    _state: Arc<AppState>,
    msg: &Message,
    text: &str,
) -> Result<()> {
    let (cmd, _arg) = parse_command(text);

    match cmd {
        "start" => {
            bot.send_message(
                msg.chat.id,
                "Привет! Я Telegram Ranger 👋\nДобавь меня в группу — включу капчу для новых участников."
            ).await?;
        }
        "help" => {
            bot.send_message(msg.chat.id, "❌ Команды доступны только администратору.")
                .await?;
        }
        _ => {
            bot.send_message(msg.chat.id, "❌ Эта команда недоступна.")
                .await?;
        }
    }

    Ok(())
}

async fn handle_admin_command(
    bot: &Bot,
    state: Arc<AppState>,
    msg: &Message,
    text: &str,
) -> Result<()> {
    let (cmd, arg) = parse_command(text);

    match cmd {
        "start" => {
            bot.send_message(
                msg.chat.id,
                "Привет! Я Telegram Ranger 👋\nДоступные команды админа:\n\
                 /allowbot <id>\n/denybot <id>\n/listbots\n/help",
            )
            .await?;
        }
        "help" => {
            bot.send_message(
                msg.chat.id,
                "Команды админа:\n/allowbot <id> — разрешить бота\n/denybot <id> — запретить бота\n/listbots — показать белый список",
            )
                .await?;
        }
        "allowbot" => {
            if let Some(id_str) = arg {
                match id_str.parse::<u64>() {
                    Ok(id) => {
                        state.allow_bot(id);
                        bot.send_message(msg.chat.id, format!("✅ Bot {id} allowed"))
                            .await?;
                    }
                    Err(_) => {
                        bot.send_message(msg.chat.id, "Usage: /allowbot <numeric_bot_id>")
                            .await?;
                    }
                }
            } else {
                bot.send_message(msg.chat.id, "Usage: /allowbot <numeric_bot_id>")
                    .await?;
            }
        }
        "denybot" => {
            if let Some(id_str) = arg {
                match id_str.parse::<u64>() {
                    Ok(id) => {
                        state.deny_bot(id);
                        bot.send_message(msg.chat.id, format!("⛔ Bot {id} denied"))
                            .await?;
                    }
                    Err(_) => {
                        bot.send_message(msg.chat.id, "Usage: /denybot <numeric_bot_id>")
                            .await?;
                    }
                }
            } else {
                bot.send_message(msg.chat.id, "Usage: /denybot <numeric_bot_id>")
                    .await?;
            }
        }
        "listbots" => {
            let mut ids: Vec<u64> = state.bot_whitelist.iter().map(|e| *e).collect();
            ids.sort_unstable();
            if ids.is_empty() {
                bot.send_message(msg.chat.id, "Белый список ботов пуст.")
                    .await?;
            } else {
                let body = ids
                    .into_iter()
                    .map(|i| i.to_string())
                    .collect::<Vec<_>>()
                    .join("\n");
                bot.send_message(msg.chat.id, format!("Разрешённые боты:\n{body}"))
                    .await?;
            }
        }
        _ => {
            // неизвестная команда — можно молча игнорировать
        }
    }

    Ok(())
}
