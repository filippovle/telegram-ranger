use crate::{captcha, commands, state::AppState};
use anyhow::Result;
use std::sync::Arc;
use teloxide::prelude::*;

pub async fn on_message(bot: Bot, state: Arc<AppState>, msg: Message) -> Result<()> {
    if let Some(text) = msg.text() {
        if text.starts_with('/') {
            // Команды доступны и в личке, и в группе; админ/не-админ различается внутри.
            commands::handle_command(&bot, state.clone(), &msg, text).await?;
            return Ok(());
        }
    }

    // Группы — поддержка legacy new_chat_members
    if (msg.chat.is_group() || msg.chat.is_supergroup()) && msg.new_chat_members().is_some() {
        if let Some(new_members) = msg.new_chat_members() {
            for u in new_members {
                captcha::ask_captcha(&bot, state.clone(), msg.chat.id, &u).await?;
            }
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
