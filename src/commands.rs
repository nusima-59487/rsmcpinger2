use crate::{
    SERVER_DATA_ROOT_DIR,
    data_handler::ServerData,
    err::{Error, ErrorCause},
};
use chrono::Utc;
use poise::{
    CreateReply,
    serenity_prelude::{
        Colour, ComponentInteractionCollector, CreateActionRow, CreateButton, CreateEmbed,
        CreateInteractionResponse, CreateInteractionResponseMessage,
    },
};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

type CommandError = Box<dyn std::error::Error + Send + Sync>;
type Context<'a> = poise::Context<'a, crate::Data, CommandError>;

/// Pong!
#[poise::command(slash_command, prefix_command)]
pub async fn ping(ctx: Context<'_>) -> Result<(), CommandError> {
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
pub async fn setup(
    ctx: Context<'_>,
    #[description = "Minecraft server address"] server_address: String,
    #[description = "Minecraft server port (defaults to 25565)"] server_port: Option<u16>,
    #[description = "[UNUSED FOR NOW] Minecraft RCON port (defaults to 25575)"] rcon_port: Option<
        u16,
    >,
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
    let msg_reply_handle = match server_data.save() {
        Ok(_) => {
            ctx.say("Successfully set up!").await?
            
        }
        Err(why) => {
            ctx.send(CreateReply::default().embed(why.get_embed())).await?
        }
    };
    tokio::time::sleep(Duration::from_secs(5)).await; 
    msg_reply_handle.delete(ctx).await?; 
    
    return Ok(()); 
}

/// [Admin] Removes all players' online record seen in /playtime player
#[poise::command(
    slash_command,
    prefix_command,
    guild_only,
    default_member_permissions = "ADMINISTRATOR"
)]
pub async fn remove_player_records(ctx: Context<'_>) -> Result<(), CommandError> {
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
    let mut server_data = server_data_result.unwrap();

    server_data.reset_all_player_data();

    match server_data.save() {
        Ok(_) => {
            ctx.say("All player data erased!").await?; 
        }, 
        Err(why) => {
            ctx.send(CreateReply::default().embed(why.get_embed())).await?; 
        }
    }
    return Ok(());
}

/// [Admin] removes minecraft server data for this channel
#[poise::command(
    slash_command,
    prefix_command,
    guild_only,
    default_member_permissions = "ADMINISTRATOR"
)]
pub async fn remove_server_data(ctx: Context<'_>) -> Result<(), CommandError> {
    ctx.defer().await?;

    let Some(guild_id) = ctx.guild_id() else {
        ctx.say("HOW DARE YOU USE THIS COMMAND OUTSIDE OF A SERVER >:(")
            .await?;
        return Ok(());
    };
    let filename = format!("{SERVER_DATA_ROOT_DIR}/{guild_id}.json");

    match std::fs::remove_file(filename){
        Ok(_) => {
            ctx.say("Server data deleted!").await?; 
        }, 
        Err(why) => {
            let embed = Error {
                cause: ErrorCause::ServerDataDel, 
                reason: why.to_string()
            }.get_embed(); 
            ctx.send(CreateReply::default().embed(embed)).await?; 
        }
    };
    return Ok(());
}

/// Check online players
#[poise::command(slash_command, prefix_command, guild_only)]
pub async fn playerlist(ctx: Context<'_>) -> Result<(), CommandError> {
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
pub async fn playtime(_ctx: Context<'_>) -> Result<(), CommandError> {
    Ok(())
}

/// Shows playtime leaderboard of the server, sorted by playtime in descending order
#[poise::command(slash_command, prefix_command, rename = "leaderboard")]
pub async fn playtime_leaderboard(ctx: Context<'_>) -> Result<(), CommandError> {
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
pub async fn playtime_player(
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

    let mut online_records = player_data
        .online_record
        .clone()
        .into_iter()
        .rev()
        .map(|record| record.to_string())
        .collect::<Vec<_>>();
    if player_data.is_online {
        online_records.insert(
            0,
            format!(
                "- **<t:{}:s>**\n> :green_circle: *Currently online!*",
                chrono::DateTime::parse_from_rfc3339(&player_data.last_seen)
                    .map(|e| e.with_timezone(&Utc).timestamp())
                    .map_err(|e| Error {
                        cause: ErrorCause::DateTimeParse,
                        reason: e.to_string(),
                    })
                    // ?,
                    .unwrap_or_default(),
            ),
        );
    }
    // .insert(0, element)

    let total_page_count = online_records.len().div_ceil(5).max(1);

    let ctx_id = ctx.id();
    let prev_button_id = format!("{}prev", ctx_id);
    let next_button_id = format!("{}next", ctx_id);

    let mut current_page_idx = 0;

    let reply = {
        let components = CreateActionRow::Buttons(vec![
            CreateButton::new(&prev_button_id).emoji('◀'),
            CreateButton::new(&next_button_id).emoji('▶'),
        ]);

        CreateReply::default()
            .embed(generate_playtime_player_embed(
                &player_name,
                player_data.get_current_online_secs(),
                &online_records,
                current_page_idx,
            ))
            .components(vec![components])
    };
    ctx.send(reply).await?;

    while let Some(press) = ComponentInteractionCollector::new(ctx)
        // We defined our button IDs to start with `ctx_id`. If they don't, some other command's
        // button was pressed
        .filter(move |press| press.data.custom_id.starts_with(&ctx_id.to_string()))
        // Timeout when no navigation button has been pressed for 5 minutes
        .timeout(std::time::Duration::from_secs(60 * 25))
        .await
    {
        // Depending on which button was pressed, go to next or previous page
        if press.data.custom_id == next_button_id {
            current_page_idx += 1;
            if current_page_idx >= total_page_count {
                current_page_idx = 0;
            }
        } else if press.data.custom_id == prev_button_id {
            current_page_idx = current_page_idx
                .checked_sub(1)
                .unwrap_or(total_page_count - 1);
        } else {
            // This is an unrelated button interaction
            continue;
        }

        // Update the message with the new page contents
        press
            .create_response(
                ctx.serenity_context(),
                CreateInteractionResponse::UpdateMessage(
                    CreateInteractionResponseMessage::new().embed(generate_playtime_player_embed(
                        &player_name,
                        player_data.get_current_online_secs(),
                        &online_records,
                        current_page_idx,
                    )),
                ),
            )
            .await?;
    }

    return Ok(());
}

fn generate_playtime_player_embed(
    player_name: &str,
    total_playtime_secs: u32,
    online_record_descs: &Vec<String>,
    page_idx: usize,
) -> CreateEmbed {
    let online_records = online_record_descs.chunks(5).collect::<Vec<_>>();
    let pages_count = online_records.len().max(1);
    let page_idx = page_idx.min(pages_count - 1);
    // let page_online_records = online_records[page_idx].join("\n");
    let page_online_records = online_records
        .get(page_idx)
        .map(|records| records.join("\n"))
        .unwrap_or("No playtime records found!".into());

    let embed = CreateEmbed::new()
        .title(format!("⌛  Playtime Info on {}", player_name))
        .colour(Colour::FABLED_PINK);
    let embed = if page_idx == 0 {
        embed.field(
            "Total Playtime",
            format!(
                "```{}h {}m {}s```",
                total_playtime_secs / 3600,
                (total_playtime_secs % 3600) / 60,
                total_playtime_secs % 60,
            ),
            false,
        )
    } else {
        embed
    };
    let embed = embed.field(
        format!("Playtime History (Page {} of {})", page_idx + 1, pages_count),
        page_online_records,
        false,
    );
    return embed;
}
