use crate::{bot_logics::Listener, data_handler::ServerData, err::ErrorCause};
use chrono::Utc;
use poise::{
    CreateReply,
    serenity_prelude::{self as serenity, Colour, CreateEmbed, HttpBuilder},
};
use std::{
    sync::{Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

mod bot_logics;
mod data_handler;
mod err;
mod pinger;

struct Data {} // User data, which is stored and accessible in all command invocations
type CommandError = Box<dyn std::error::Error + Send + Sync>;
type Context<'a> = poise::Context<'a, Data, CommandError>;

const SERVER_DATA_ROOT_DIR: &str = "./serverdata";
const PING_INTERVAL_SECS: u64 = 30;
const RCON_TIME_LIMIT_SECS: u64 = 2;

/// Pong!
#[poise::command(slash_command, prefix_command)]
async fn ping(ctx: Context<'_>) -> Result<(), CommandError> {
    let message_sent_time = ctx.created_at().timestamp_millis() as u128;
    let current_time = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis();
    let time_diff = current_time - message_sent_time;

    let color: Colour = match time_diff {
        0..100 => Colour::DARK_GREEN,
        100..500 => Colour::ORANGE,
        500.. => Colour::RED,
    };

    ctx.send(
        CreateReply::default().embed(
            CreateEmbed::new()
                .title("🏓  Pong!")
                .description(format!("Delay is {}ms", time_diff))
                .color(color),
        ),
    )
    .await?;
    Ok(())
}

/// [Admin] Set up minecraft server data for this channel
#[poise::command(
    slash_command,
    prefix_command,
    guild_only,
    default_member_permissions = "ADMINISTRATOR"
)]
async fn setup(
    ctx: Context<'_>,
    #[description = "Minecraft server address"] server_address: String,
    #[description = "Minecraft server port (defaults to 25565)"] server_port: Option<u16>,
    #[description = "[UNUSED FOR NOW] Minecraft RCON port (defaults to 25575)"] rcon_port: Option<u16>,
    #[description = "Minecraft RCON password"] rcon_password: String,
) -> Result<(), CommandError> {
    let Some(guild_id) = ctx.guild_id() else {
        ctx.say("HOW DARE YOU USE THIS COMMAND OUTSIDE OF A SERVER >:(")
            .await?;
        return Ok(());
    };
    let server_data = ServerData::new(
        server_address,
        server_port.unwrap_or(25565),
        rcon_port.unwrap_or(25575),
        rcon_password,
        SERVER_DATA_ROOT_DIR,
        ctx.channel_id().get(),
        guild_id.get(),
    );
    match server_data.save() {
        Ok(_) => {
            ctx.say("Successfully set up!").await?;
            return Ok(());
        }
        Err(why) => {
            ctx.send(CreateReply::default().embed(why.get_embed()))
                .await?;
            return Ok(());
        }
    };
}

/// Check online players
#[poise::command(slash_command, prefix_command, guild_only)]
async fn playerlist(ctx: Context<'_>) -> Result<(), CommandError> {
    ctx.defer().await?;

    let Some(guild_id) = ctx.guild_id() else {
        ctx.say("HOW DARE YOU USE THIS COMMAND OUTSIDE OF A SERVER >:(")
            .await?;
        return Ok(());
    };
    let guild_id = guild_id.get();
    let server_data_result = ServerData::read(SERVER_DATA_ROOT_DIR, guild_id);
    if let Err(why) = server_data_result {
        if let ErrorCause::ServerDataRead = why.cause {
            ctx.say("Error: Server haven't set up yet!").await?;
        } else {
            ctx.send(CreateReply::default().embed(why.get_embed()))
                .await?;
        }
        return Ok(());
    }
    let server_data = server_data_result.unwrap();

    // TODO: fix ts
    let result = server_data.fetch_online_players_list().await;
    match result {
        Ok(player_vec) => {
            let players_count = player_vec.len();
            let message = match players_count {
                0 => "**No players online**".into(),
                1 if player_vec[0].is_empty() => "**No players online**".into(),
                _ => format!(
                    "**{} player(s) online:**\n- `{}`",
                    players_count,
                    player_vec.join("`\n- `")
                ),
            };
            ctx.say(message).await?;
            return Ok(());
        }
        Err(why) => {
            ctx.send(CreateReply::default().embed(why.get_embed()))
                .await?;
            return Ok(());
        }
    }
}

#[poise::command(
    slash_command,
    prefix_command,
    subcommands("playtime_leaderboard", "playtime_player")
)]
async fn playtime(_ctx: Context<'_>) -> Result<(), CommandError> {
    Ok(())
}

/// Shows playtime leaderboard of the server, sorted by playtime in descending order
#[poise::command(slash_command, prefix_command, rename = "leaderboard")]
async fn playtime_leaderboard(ctx: Context<'_>) -> Result<(), CommandError> {
    ctx.defer().await?;
    let Some(guild_id) = ctx.guild_id() else {
        ctx.say("HOW DARE YOU USE THIS COMMAND OUTSIDE OF A SERVER >:(")
            .await?;
        return Ok(());
    };
    let guild_id = guild_id.get();
    let server_data_result = ServerData::read(SERVER_DATA_ROOT_DIR, guild_id);
    if let Err(why) = server_data_result {
        if let ErrorCause::ServerDataRead = why.cause {
            ctx.say("Error: Server haven't set up yet!").await?;
        } else {
            ctx.send(CreateReply::default().embed(why.get_embed()))
                .await?;
        }
        return Ok(());
    }
    let server_data = server_data_result.unwrap();

    let mut players_data_vec: Vec<_> = server_data.player_data.iter().collect();
    players_data_vec.sort_by(|(_, player1), (_, player2)| {
        player2
            .get_current_online_secs()
            .cmp(&player1.get_current_online_secs())
    });
    let playtime_entries = players_data_vec
        .iter()
        .enumerate()
        .map(|(idx, (player_name, player_data))| {
            format!(
                "{}. `{}` - `{}`h `{}`m `{}`s",
                idx,
                player_name,
                player_data.get_current_online_secs() / 3600,
                (player_data.get_current_online_secs() % 3600) / 60,
                player_data.get_current_online_secs() % 60
            )
        })
        .collect::<Vec<_>>();

    let embed_to_return = CreateEmbed::new()
        .title("👑  Playtime Leaderboard")
        .description(playtime_entries.join("\n"))
        .color(Colour::FABLED_PINK);
    ctx.send(CreateReply::default().embed(embed_to_return))
        .await?;
    return Ok(());
}

/// Shows playtime data of a player
#[poise::command(slash_command, prefix_command, rename = "player")]
async fn playtime_player(
    ctx: Context<'_>,
    #[description = "Player name to check"] player_name: String,
) -> Result<(), CommandError> {
    ctx.defer().await?;
    let Some(guild_id) = ctx.guild_id() else {
        ctx.say("HOW DARE YOU USE THIS COMMAND OUTSIDE OF A SERVER >:(")
            .await?;
        return Ok(());
    };
    let guild_id = guild_id.get();
    let server_data_result = ServerData::read(SERVER_DATA_ROOT_DIR, guild_id);
    if let Err(why) = server_data_result {
        if let ErrorCause::ServerDataRead = why.cause {
            ctx.say("Error: Server haven't set up yet!").await?;
        } else {
            ctx.send(CreateReply::default().embed(why.get_embed()))
                .await?;
        }
        return Ok(());
    }
    let server_data = server_data_result.unwrap();

    let Some(player_data) = server_data.get_player_data(&player_name) else {
        ctx.say("Player not found!").await?;
        return Ok(());
    };
    let embed_desc = if player_data.is_online {
        format!(
            "Total Playtime: `{}`h `{}`m `{}`s\nJoined server: <t:{}>",
            player_data.get_current_online_secs() / 3600,
            (player_data.get_current_online_secs() % 3600) / 60,
            player_data.get_current_online_secs() % 60,
            chrono::DateTime::parse_from_rfc3339(&player_data.last_seen)
                .map(|e| e.with_timezone(&Utc))
                .unwrap_or_else(|_| chrono::DateTime::from(UNIX_EPOCH))
                .timestamp(),
        )
    } else {
        format!(
            "Total Playtime: `{}`h `{}`m `{}`s\nLast online: <t:{}>",
            player_data.get_current_online_secs() / 3600,
            (player_data.get_current_online_secs() % 3600) / 60,
            player_data.get_current_online_secs() % 60,
            
            chrono::DateTime::parse_from_rfc3339(&player_data.last_seen)
                .map(|e| e.with_timezone(&Utc))
                .unwrap_or_else(|_| chrono::DateTime::from(UNIX_EPOCH))
                .timestamp(),
        )
    };
    let embed_to_return = CreateEmbed::new()
        .title(format!("⌛  Playtime Info on {}", player_name))
        .description(embed_desc)
        .color(Colour::FABLED_PINK);
    ctx.send(CreateReply::default().embed(embed_to_return))
        .await?;
    return Ok(());
}

#[tokio::main(flavor = "current_thread")]
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

    let http = HttpBuilder::new(&token)
        .proxy(proxy)
        .build();
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
