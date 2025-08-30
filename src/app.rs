use crate::{captcha, config::Config, handlers, state::AppState};
use anyhow::Result;
use dotenvy::dotenv;
use log::info;
use std::sync::Arc;
use teloxide::{dptree, prelude::*};

pub async fn run() -> Result<()> {
    dotenv().ok();
    pretty_env_logger::init();

    let bot = Bot::from_env();
    // На всякий случай — polling-only.
    bot.delete_webhook().await.ok();

    let cfg = Config::from_env();
    let state = Arc::new(AppState::new(cfg));

    info!("Starting telegram-ranger…");

    // Регистрация хендлеров.
    let handler = dptree::entry()
        .branch(Update::filter_message().endpoint(handlers::on_message))
        .branch(Update::filter_chat_member().endpoint(handlers::on_chat_member_update))
        .branch(Update::filter_callback_query().endpoint(captcha::on_callback));

    // Прокидываем зависимости в дерево.
    Dispatcher::builder(bot, handler)
        .dependencies(dptree::deps![state])
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;

    Ok(())
}
