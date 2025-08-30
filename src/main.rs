mod app;
mod config;
mod state;
mod handlers;
mod captcha;
mod commands;
mod utils;

use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    app::run().await
}