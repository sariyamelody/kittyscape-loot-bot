use anyhow::Result;
use serenity::all::{
    CommandInteraction,
    CreateInteractionResponse,
    CreateInteractionResponseMessage,
};
use sqlx::SqlitePool;
use crate::logger;

pub async fn handle_rsname_remove(
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
    
    // Check if this RS name is linked to the user
    let result = sqlx::query!(
        "DELETE FROM runescape_accounts 
         WHERE discord_id = ? AND runescape_name = ?",
        discord_id,
        rs_name
    )
    .execute(db)
    .await?;
    
    // Get all RS names linked to this Discord account after the removal
    let existing = sqlx::query!(
        "SELECT runescape_name FROM runescape_accounts WHERE discord_id = ?",
        discord_id
    )
    .fetch_all(db)
    .await?
    .into_iter()
    .filter_map(|record| Some(record.runescape_name))
    .collect::<Vec<String>>();
    
    // Log the action if we actually removed something
    if result.rows_affected() > 0 {
        logger::log_action(
            ctx,
            &discord_id,
            "UNLINKED RSNAME",
            &format!("{}", rs_name)
        ).await?;
    }
    
    // Create response message
    let message = if result.rows_affected() > 0 {
        if existing.is_empty() {
            format!("Successfully unlinked RuneScape account '{}' from your Discord account.\n\nYou have no more linked accounts.", rs_name)
        } else {
            format!("Successfully unlinked RuneScape account '{}' from your Discord account.\n\nYour remaining linked accounts: {}", 
                    rs_name, existing.join(", "))
        }
    } else {
        if existing.is_empty() {
            "This RuneScape account is not linked to your Discord account.\n\nYou have no linked accounts.".to_string()
        } else {
            format!("This RuneScape account is not linked to your Discord account.\n\nYour linked accounts: {}", 
                    existing.join(", "))
        }
    };
    
    command
        .create_response(&ctx.http, CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new().content(message)
        ))
        .await?;
    
    Ok(())
} 