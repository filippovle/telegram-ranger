// src/utils.rs
use teloxide::utils::html::escape;
use teloxide::{prelude::*, types::UserId};

/// Возвращает строку для упоминания пользователя:
/// - Если есть @username — вернёт "@username".
/// - Иначе — HTML-ссылку вида <a href="tg://user?id=...">Имя Фамилия</a>.
///
/// ВНИМАНИЕ: строку следует отправлять с ParseMode::Html.
///
/// Не ломает совместимость: сигнатура и внешний контракт прежние.
pub async fn mention(bot: &Bot, chat_id: ChatId, user_id: UserId) -> String {
    match bot.get_chat_member(chat_id, user_id).await {
        Ok(cm) => {
            let u = cm.user;
            if let Some(username) = u.username.as_deref() {
                format!("@{username}")
            } else {
                let display = match u.last_name {
                    Some(last) => format!("{} {}", u.first_name, last),
                    None => u.first_name,
                };
                format_mention_link(u.id, &display)
            }
        }
        // Фолбэк: если запрос к API не удался — даём кликабельную ссылку с user_id
        Err(_) => format_mention_link(user_id, &user_id.0.to_string()),
    }
}

/// Приведение "@Name" -> "name" (lower-case, без '@').
pub fn normalize_username<S: AsRef<str>>(name: S) -> String {
    let n = name.as_ref().trim();
    let n = n.strip_prefix('@').unwrap_or(n);
    n.to_lowercase()
}

/// Приватный помощник: формирует безопасную HTML-ссылку-упоминание.
fn format_mention_link(user_id: UserId, display_name: &str) -> String {
    // Экранируем отображаемое имя на случай спецсимволов.
    format!(
        r#"<a href="tg://user?id={}">{}</a>"#,
        user_id.0,
        escape(display_name)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_username() {
        assert_eq!(normalize_username("@Name"), "name");
        assert_eq!(normalize_username("  @User_Name  "), "user_name");
        assert_eq!(normalize_username("Plain"), "plain");
        assert_eq!(normalize_username(""), "");
    }

    #[test]
    fn test_format_mention_link_html_escape() {
        let uid = UserId(123);
        let s = format_mention_link(uid, r#"Alice & Bob <3"#);
        assert!(s.contains(r#"Alice &amp; Bob &lt;3"#));
        assert!(s.contains(r#"tg://user?id=123"#));
    }
}
