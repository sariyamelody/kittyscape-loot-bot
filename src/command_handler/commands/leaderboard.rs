use anyhow::Result;
use serenity::all::{
    CommandInteraction,
    CreateInteractionResponse,
    CreateInteractionResponseMessage,
};
use sqlx::SqlitePool;
use crate::command_handler::{format_points, format_number};

pub async fn handle_leaderboard(
    command: &CommandInteraction,
    ctx: &serenity::prelude::Context,
    db: &SqlitePool,
) -> Result<()> {
    let top_users = sqlx::query!(
        "SELECT u.discord_id, u.points, u.total_drops,
                (SELECT COUNT(*) FROM collection_log_entries WHERE discord_id = u.discord_id) as total_clogs
         FROM users u
         ORDER BY u.points DESC 
         LIMIT 10"
    )
    .fetch_all(db)
    .await?;

    let mut message_content = String::from("🏆 **Top 10 Looters** 🏆\n");
    for (i, user) in top_users.iter().enumerate() {
        message_content.push_str(&format!(
            "{}. <@{}> - {} ({} drops, {} clogs)\n",
            i + 1,
            user.discord_id.as_ref().unwrap_or(&"Unknown".to_string()),
            format_points(user.points),
            format_number(user.total_drops),
            format_number(user.total_clogs.unwrap_or(0) as i64)
        ));
    }

    command
        .create_response(&ctx.http, CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new().content(message_content)
        ))
        .await?;

    Ok(())
} 