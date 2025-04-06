use anyhow::Result;
use serenity::all::{
    CommandInteraction,
    CreateInteractionResponse,
    CreateInteractionResponseMessage,
};
use sqlx::SqlitePool;
use crate::logger;

pub async fn handle_rsname(
    command: &CommandInteraction,
    ctx: &serenity::prelude::Context,
    db: &SqlitePool,
) -> Result<()> {
    let options = &command.data.options;
    
    let rs_name = options
        .iter()
        .find(|opt| opt.name == "username")
        .and_then(|opt| opt.value.as_str())
        .ok_or_else(|| anyhow::anyhow!("RuneScape username not provided"))?;

    let discord_id = command.user.id.to_string();
    
    // Insert or update user in users table if they don't exist
    sqlx::query!(
        "INSERT INTO users (discord_id, points, total_drops) 
         VALUES (?, 0, 0)
         ON CONFLICT(discord_id) DO NOTHING",
        discord_id
    )
    .execute(db)
    .await?;
    
    // Link RS name to Discord ID
    let result = sqlx::query!(
        "INSERT INTO runescape_accounts (discord_id, runescape_name) 
         VALUES (?, ?)
         ON CONFLICT(discord_id, runescape_name) DO NOTHING",
        discord_id,
        rs_name
    )
    .execute(db)
    .await?;
    
    // Get all RS names linked to this Discord account
    let existing = sqlx::query!(
        "SELECT runescape_name FROM runescape_accounts WHERE discord_id = ?",
        discord_id
    )
    .fetch_all(db)
    .await?
    .into_iter()
    .filter_map(|record| Some(record.runescape_name))
    .collect::<Vec<String>>();
    
    // Log the action
    logger::log_action(
        ctx,
        &discord_id,
        "LINKED RSNAME",
        &format!("{}", rs_name)
    ).await?;
    
    // Create response message
    let message = if result.rows_affected() > 0 {
        format!("Successfully linked RuneScape account '{}' to your Discord account.\n\nYour linked accounts: {}", 
                rs_name, existing.join(", "))
    } else {
        format!("This RuneScape account is already linked to your Discord account.\n\nYour linked accounts: {}", 
                existing.join(", "))
    };
    
    command
        .create_response(&ctx.http, CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new().content(message)
        ))
        .await?;
    
    Ok(())
} 