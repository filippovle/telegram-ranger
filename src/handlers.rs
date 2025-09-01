use crate::{captcha, commands, state::AppState};
use anyhow::Result;
use std::sync::Arc;
use teloxide::prelude::*;

pub async fn on_message(bot: Bot, state: Arc<AppState>, msg: Message) -> Result<()> {
    // фиксируем сообщения тех, кто уже ждёт капчу
    if let (Some(from), chat) = (msg.from.as_ref(), &msg.chat) {
        let key = AppState::key(chat.id, from.id);
        if let Some(mut pending) = state.pending.get_mut(&key) {
            // не трекаем служебные события без msg.id
            if msg.id.0 != 0 {
                pending.user_message_ids.push(msg.id.0);
            }
        }
    }

    // команды (и в личке, и в группе)
    if let Some(text) = msg.text() {
        if text.starts_with('/') {
            // важно: передаём &msg, не двигаем его
            commands::handle_command(&bot, state.clone(), &msg, text).await?;
            return Ok(());
        }
    }

    Ok(())
}


pub async fn on_chat_member_update(
    bot: Bot,
    state: Arc<AppState>,
    upd: ChatMemberUpdated,
) -> Result<()> {
    // основной путь вступления
    let became_present = upd.new_chat_member.is_present();
    let was_absent = !upd.old_chat_member.is_present();

    if became_present && was_absent {
        let chat_id = upd.chat.id;
        let user = &upd.new_chat_member.user;
        captcha::ask_captcha(&bot, state, chat_id, user).await?;
    }
    Ok(())
}
