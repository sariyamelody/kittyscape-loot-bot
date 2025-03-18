use anyhow::Result;
use serenity::all::{
    CommandInteraction,
    CreateInteractionResponse,
    CreateInteractionResponseMessage,
};
use sqlx::SqlitePool;

pub async fn handle_points(
    command: &CommandInteraction,
    ctx: &serenity::prelude::Context,
    db: &SqlitePool,
) -> Result<()> {
    let discord_id = command.user.id.to_string();
    let user_data = sqlx::query!(
        "SELECT points, total_drops FROM users WHERE discord_id = ?",
        discord_id
    )
    .fetch_optional(db)
    .await?;

    let message_content = match user_data {
        Some(data) => {
            format!(
                "You have {} points from {} total drops!",
                data.points, data.total_drops
            )
        }
        None => "You haven't recorded any drops yet!".to_string(),
    };

    command
        .create_response(&ctx.http, CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new().content(message_content)
        ))
        .await?;

    Ok(())
} 