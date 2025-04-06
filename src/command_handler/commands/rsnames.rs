use anyhow::Result;
use serenity::all::{
    CommandInteraction,
    CreateInteractionResponse,
    CreateInteractionResponseMessage,
};
use sqlx::SqlitePool;

pub async fn handle_rsnames(
    command: &CommandInteraction,
    ctx: &serenity::prelude::Context,
    db: &SqlitePool,
) -> Result<()> {
    let discord_id = command.user.id.to_string();
    
    // Get all RS names linked to this Discord account
    let linked_accounts = sqlx::query!(
        "SELECT runescape_name, timestamp FROM runescape_accounts 
         WHERE discord_id = ? 
         ORDER BY timestamp DESC",
        discord_id
    )
    .fetch_all(db)
    .await?;
    
    // Create response message
    if linked_accounts.is_empty() {
        command
            .create_response(&ctx.http, CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new()
                    .content("You don't have any RuneScape accounts linked to your Discord account yet.\n\nUse `/rsname username:YourRSName` to link an account.")
            ))
            .await?;
    } else {
        let mut message = "Your linked RuneScape accounts:\n".to_string();
        
        for (i, account) in linked_accounts.iter().enumerate() {
            let rs_name = &account.runescape_name;
            let display_name = if rs_name.is_empty() { "Unknown" } else { rs_name };
            let timestamp = account.timestamp.map(|ts| ts.format("%B %d, %Y").to_string()).unwrap_or_else(|| "Unknown date".to_string());
            
            message.push_str(&format!("{}. **{}** (linked on {})\n", i + 1, display_name, timestamp));
        }
        
        message.push_str("\nYou can unlink accounts with `/rsname_remove username:RSName`");
        
        command
            .create_response(&ctx.http, CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new().content(message)
            ))
            .await?;
    }
    
    Ok(())
} 