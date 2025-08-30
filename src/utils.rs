// src/utils.rs
use teloxide::{prelude::*, types::UserId};

pub async fn mention(bot: &Bot, chat_id: ChatId, user_id: UserId) -> String {
    if let Ok(cm) = bot.get_chat_member(chat_id, user_id).await {
        let u = cm.user;
        if let Some(username) = u.username {
            format!("@{username}")
        } else {
            format!("<a href=\"tg://user?id={}\">{}</a>", u.id.0, u.first_name)
        }
    } else {
        format!("<a href=\"tg://user?id={}\">{}</a>", user_id.0, user_id.0)
    }
}
