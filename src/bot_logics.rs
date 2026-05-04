use crate::err::ErrorCause;
use crate::{
    PING_INTERVAL_SECS, SERVER_DATA_ROOT_DIR,
    data_handler::{ServerData, get_all_server_data},
    err::Error,
};
use poise::serenity_prelude::{
    ChannelId, Colour, Context, CreateEmbed, CreateMessage, EventHandler, MessageFlags, Ready,
    async_trait,
};
use std::sync::Mutex;
use std::{
    collections::HashMap,
    time::Duration,
};
use tokio::time::sleep;

/// i don't have an utils file so im putting it here
fn escape_player_name(player_name: &String) -> String {
    player_name
        .chars()
        .map(|c| {
            if c.is_ascii_punctuation() {
                format!("\\{}", c)
            } else {
                c.to_string()
            }
        })
        .collect::<String>()
}

async fn server_pinger_logic(bot_ctx: &Context, server_data: &mut ServerData) -> Result<(), Error> {
    let mut embeds_to_return: Vec<CreateEmbed> = vec![];

    let online_status_result = server_data.fetch_slp().await;
    let is_server_online = match online_status_result {
        Ok(_val) => true,
        Err(Error { cause, .. }) if matches!(cause, ErrorCause::SlpConn) => false,
        Err(why) => {
            return Err(why);
        }
    };
    let online_players_data: HashMap<_, _> = server_data
        .player_data
        .iter()
        .filter(|(_, data)| data.is_online)
        .collect();
    let old_online_players_count = online_players_data.len() as u64;
    let new_online_players_count = server_data.fetch_online_players_count().await.ok();

    if server_data.is_online != is_server_online {
        server_data.set_online(is_server_online);
        embeds_to_return.push(match is_server_online {
            true => CreateEmbed::default()
                .title("✅ The server is back online!")
                .colour(Colour::DARK_GREEN),
            false => CreateEmbed::default()
                .title("❌ The server is offline...")
                .colour(Colour::DARK_RED),
        });
    } else if is_server_online
        && let Some(new_online_players_count) = new_online_players_count
        && old_online_players_count != new_online_players_count
    {
        let mut players_data_to_update: HashMap<_, _> = HashMap::new();

        let old_online_players: Vec<&String> = online_players_data.keys().cloned().collect();
        let new_online_players = server_data.fetch_online_players_list().await?;

        let left_players: Vec<_> = old_online_players
            .iter()
            .filter(|x| !new_online_players.contains(x) && ***x != String::from(""))
            .cloned()
            .collect();
        let joined_players: Vec<_> = new_online_players
            .iter()
            .filter(|x| !old_online_players.contains(x) && ***x != String::from(""))
            .cloned()
            .collect();

        for player in left_players {
            let escaped_player_name = escape_player_name(player);
            // update embeds
            embeds_to_return.push(
                CreateEmbed::default()
                    .title(format!(
                        "➖  **{}** left the server *({})*",
                        escaped_player_name, new_online_players_count
                    ))
                    .colour(Colour::ORANGE),
            );
            // insert player data to update online status later
            let mut player_data = server_data
                .get_player_data(player)
                .map(|data| data.clone())
                .unwrap_or_default();
            player_data.set_online(false);
            players_data_to_update.insert(player.to_string(), player_data);
        }
        for player in joined_players {
            let escaped_player_name = escape_player_name(&player);
            // update embeds
            embeds_to_return.push(
                CreateEmbed::default()
                    .title(format!(
                        "➕  **{}** joined the server *({})*",
                        escaped_player_name, new_online_players_count
                    ))
                    .colour(Colour::BLUE),
            );
            // insert player data to update online status later
            let mut player_data = server_data
                .get_player_data(&player)
                .map(|data| data.clone())
                .unwrap_or_default();
            player_data.set_online(true);
            players_data_to_update.insert(player.to_string(), player_data);
        }

        // update player data online status
        for (player_name, player_data) in players_data_to_update.into_iter() {
            server_data.set_player_data(&player_name, player_data);
        }
    }

    

    if embeds_to_return.is_empty() {
        server_data.save()?;
        return Ok(());
    }

    // delete old status message
    if let Some(old_status_msg_id) = server_data.status_message_id {
        let _ = ChannelId::new(server_data.status_channel_id)
            .delete_message(bot_ctx, old_status_msg_id)
            .await;
    }
    // send new embeds
    let _ = ChannelId::new(server_data.status_channel_id)
        .send_message(bot_ctx, CreateMessage::new().embeds(embeds_to_return))
        .await;
    // send new status message
    let currently_online_players = server_data
        .player_data
        .iter()
        .filter(|(playername, data)| data.is_online && *playername != "")
        .map(|(key, _)| key.clone())
        .collect::<Vec<_>>();
    let status_msg_desc = match new_online_players_count {
        Some(0) => "*No one is online right now!*".to_string(),
        Some(_count) => format!("- `{}`", currently_online_players.join("`\n- `")),
        None => "*The server is offline!*".to_string(),
    };
    let status_message_result = ChannelId::new(server_data.status_channel_id)
        .send_message(
            bot_ctx,
            CreateMessage::new()
                .flags(MessageFlags::SUPPRESS_NOTIFICATIONS)
                .embed(
                    CreateEmbed::default()
                        .title(format!(
                            ":speaking_head:  Online Players ({})",
                            new_online_players_count.map_or(-1, |count| count as i64)
                        ))
                        .description(status_msg_desc)
                        .colour(Colour::DARK_GREY),
                ),
        )
        .await;

    server_data.status_message_id = status_message_result.ok().map(|msg| msg.id);

    server_data.save()?;
    return Ok(());
}

async fn new_bot_pinger_logic(bot_ctx: Context) {
    loop {
        let server_datas_result = get_all_server_data(SERVER_DATA_ROOT_DIR);
        if let Err(why) = server_datas_result {
            eprintln!("{} ({})", why.cause.to_string(), why.reason);
            return;
        }
        let mut server_datas = server_datas_result.unwrap();
        for (_, server_data) in server_datas.iter_mut() {
            if let Err(e) = server_pinger_logic(&bot_ctx, server_data).await {
                let _ = ChannelId::new(server_data.status_channel_id)
                    .send_message(&bot_ctx, CreateMessage::new().embed(e.get_embed()))
                    .await
                    .inspect_err(|e| eprintln!("{:?}", e));
                // let's bomb the dev w/ error messages
                let _ = ChannelId::new(1490505975315042304)
                    .send_message(&bot_ctx, CreateMessage::new().embed(e.get_embed()))
                    .await
                    .inspect_err(|e| eprintln!("{:?}", e));
            };
        }

        sleep(Duration::from_secs(PING_INTERVAL_SECS)).await;
    }
}

pub struct Listener {
    // pub(crate) is_pinger_running: AtomicBool,
    pub(crate) existing_handle: Mutex<Option<tokio::task::JoinHandle<()>>>,
}

#[async_trait]
impl EventHandler for Listener {
    async fn ready(&self, ctx: Context, _ready: Ready) {
        println!("Bot is ready!");
        { // mutex lock scope
            let Ok(mut handle_lock) = self.existing_handle.lock() else {
                eprintln!("Failed to acquire lock for pinger handle");
                return;
            };
            if handle_lock.is_some() {
                println!("Pinger is already running, skipping...");
                return;
            }
            let handle = tokio::spawn(async move {
                new_bot_pinger_logic(ctx.clone()).await; 
            }); 
            *handle_lock = Some(handle);
        }
        println!("Pinger task started.");
    }
}
