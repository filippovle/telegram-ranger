use crate::{captcha, commands, state::AppState};
use anyhow::Result;
use std::sync::Arc;
use teloxide::prelude::*;

pub async fn on_message(bot: Bot, state: Arc<AppState>, msg: Message) -> Result<()> {
    // 1) трекаем сообщения тех, кто ждёт капчу
    if let (Some(from), chat) = (msg.from.as_ref(), &msg.chat) {
        if let Some(mut pending) = state.pending.get_mut(&AppState::key(chat.id, from.id)) {
            if msg.id.0 != 0 {
                pending.user_message_ids.push(msg.id.0);
            }
        }
    }

    // 2) fallback: сервисное сообщение о вступлении (вдруг приходит и в форуме)
    if let Some(newbies) = msg.new_chat_members() {
        for user in newbies {
            captcha::ask_captcha(&bot, state.clone(), msg.chat.id, &user).await?;
        }
        return Ok(());
    }

    // 3) дать шанс капче (math2) принять текстовый ответ
    captcha::on_user_message(bot.clone(), state.clone(), &msg).await?;

    // 4) команды
    if let Some(text) = msg.text() {
        if text.starts_with('/') {
            commands::handle_command(&bot, state.clone(), &msg, text).await?;
        }
    }
    Ok(())
}

pub async fn on_chat_member_update(
    bot: Bot,
    state: Arc<AppState>,
    upd: ChatMemberUpdated,
) -> Result<()> {
    let became_present = upd.new_chat_member.is_present();
    let was_absent = !upd.old_chat_member.is_present();

    if became_present && was_absent {
        let chat_id = upd.chat.id;
        let user = &upd.new_chat_member.user;
        captcha::ask_captcha(&bot, state, chat_id, user).await?;
    }
    Ok(())
}

// --- Диагностика (опционально). Включать feature 'diag' при сборке, чтобы не тащить в релиз. ---

#[cfg(feature = "diag")]
pub async fn log_chat_kind(bot: &Bot, chat_id: ChatId) {
    match bot.get_chat(chat_id).await {
        Ok(chat) => {
            log::info!("CHAT KIND: id={}, kind={:?}", chat_id.0, chat.kind);
        }
        Err(e) => log::warn!("Failed to get chat kind: {}", e),
    }
}

#[cfg(feature = "diag")]
pub async fn log_bot_rights(bot: &Bot, chat_id: ChatId) {
    let me = match bot.get_me().await {
        Ok(m) => m.user,
        Err(e) => {
            log::warn!("get_me failed: {}", e);
            return;
        }
    };
    match bot.get_chat_administrators(chat_id).await {
        Ok(admins) => {
            if let Some(me_admin) = admins.into_iter().find(|m| m.user.id == me.id) {
                if let teloxide::types::ChatMemberKind::Administrator(a) = me_admin.kind {
                    log::info!(
                        "BOT RIGHTS: can_restrict_members={:?}, can_delete_messages={:?}, can_invite_users={:?}",
                        a.can_restrict_members, a.can_delete_messages, a.can_invite_users
                    );
                } else {
                    log::warn!("Bot is not admin in this chat");
                }
            } else {
                log::warn!("Bot is not in admin list");
            }
        }
        Err(e) => log::warn!("Failed to get admins: {}", e),
    }
}
