mod app;
mod config;
mod state;
mod handlers;
mod commands;
mod utils;
mod captcha;

use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    app::run().await
}