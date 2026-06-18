use crate::bot_logics::Listener;
use commands::*;
use poise::serenity_prelude::{self as serenity, HttpBuilder};
use std::sync::Mutex;

mod bot_logics;
mod commands;
mod data_handler;
mod err;
mod pinger;

struct Data {} // User data, which is stored and accessible in all command invocations

const SERVER_DATA_ROOT_DIR: &str = "./serverdata";
const PING_INTERVAL_SECS: u64 = 30;
const RCON_TIME_LIMIT_SECS: u64 = 5;
const MC_SKIN_BASE_URL: &str = "https://mc-heads.net/avatar/";

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    let token = std::env::var("DISCORD_TOKEN").expect("missing DISCORD_TOKEN");
    let proxy = std::env::var("PROXY").expect("missing PROXY");
    let intents = serenity::GatewayIntents::non_privileged();

    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands: vec![ping(), playerlist(), setup(), playtime()],
            ..Default::default()
        })
        .setup(|ctx, _ready, framework| {
            Box::pin(async move {
                poise::builtins::register_globally(ctx, &framework.options().commands).await?;
                Ok(Data {})
            })
        })
        .build();

    let http = HttpBuilder::new(&token).proxy(proxy).build();
    // let client = serenity::ClientBuilder::new(token, intents)
    let client = serenity::ClientBuilder::new_with_http(http, intents)
        .framework(framework)
        .event_handler(Listener {
            existing_handle: Mutex::new(None),
        })
        .await;
    println!("Bot Started!");
    client.unwrap().start().await.unwrap();
}
