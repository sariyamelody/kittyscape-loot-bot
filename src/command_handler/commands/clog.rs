use anyhow::Result;
use serenity::all::{
    CommandInteraction,
    CreateInteractionResponse,
    CreateInteractionResponseMessage,
};
use sqlx::SqlitePool;
use crate::command_handler::{CollectionLogManagerKey, format_points, format_number};

pub async fn handle_clog(
    command: &CommandInteraction,
    ctx: &serenity::prelude::Context,
    db: &SqlitePool,
) -> Result<()> {
    let options = &command.data.options;
    
    let item_name = options
        .iter()
        .find(|opt| opt.name == "item")
        .and_then(|opt| opt.value.as_str())
        .ok_or_else(|| anyhow::anyhow!("Item name not provided"))?;

    let discord_id = command.user.id.to_string();

    // Check if user already has this collection log entry
    if let Ok(Some(existing_entry)) = sqlx::query!(
        "SELECT timestamp FROM collection_log_entries 
         WHERE discord_id = ? AND item_name = ?",
        discord_id,
        item_name
    )
    .fetch_optional(db)
    .await
    {
        let timestamp = existing_entry.timestamp.expect("Timestamp should not be null");
        command
            .create_response(&ctx.http, CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new()
                    .content(format!(
                        "You've already logged {} in your collection log on {}!",
                        item_name,
                        timestamp.format("%B %d, %Y at %H:%M UTC")
                    ))
            ))
            .await?;
        return Ok(());
    }

    // Get collection log manager from context data
    let data = ctx.data.read().await;
    let collection_log_manager = data.get::<CollectionLogManagerKey>()
        .ok_or_else(|| anyhow::anyhow!("Collection log manager not found"))?;

    // Calculate collection log points
    if let Some(points) = collection_log_manager.calculate_points(item_name).await {
        // Insert or update user
        sqlx::query!(
            "INSERT INTO users (discord_id, points, total_drops) 
             VALUES (?, 0, 0)
             ON CONFLICT(discord_id) DO NOTHING",
            discord_id
        )
        .execute(db)
        .await?;

        // Record the collection log entry
        sqlx::query!(
            "INSERT INTO collection_log_entries (discord_id, item_name, points) VALUES (?, ?, ?)",
            discord_id,
            item_name,
            points
        )
        .execute(db)
        .await?;

        // Update user points
        sqlx::query!(
            "UPDATE users 
             SET points = points + ?
             WHERE discord_id = ?",
            points,
            discord_id
        )
        .execute(db)
        .await?;

        // Check for rank up
        let user_points = sqlx::query!(
            "SELECT points FROM users WHERE discord_id = ?",
            discord_id
        )
        .fetch_one(db)
        .await?;

        // Get the next rank threshold
        let message_content = if let Ok(next_rank) = sqlx::query!(
            "SELECT points, role_name FROM rank_thresholds 
             WHERE points > ? 
             ORDER BY points ASC 
             LIMIT 1",
            user_points.points
        )
        .fetch_optional(db)
        .await
        {
            match next_rank {
                Some(rank) => {
                    format!(
                        "Collection log entry recorded: {} (+{} points)! You now have {}. Next rank at {} points for {}!",
                        item_name,
                        format_number(points),
                        format_points(user_points.points),
                        format_number(rank.points),
                        rank.role_name
                    )
                }
                None => {
                    format!(
                        "Collection log entry recorded: {} (+{} points)! You now have {}!",
                        item_name,
                        format_number(points),
                        format_points(user_points.points)
                    )
                }
            }
        } else {
            format!(
                "Collection log entry recorded: {} (+{} points)! You now have {}!",
                item_name,
                format_number(points),
                format_points(user_points.points)
            )
        };

        command
            .create_response(&ctx.http, CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new().content(message_content)
            ))
            .await?;
    } else {
        command
            .create_response(&ctx.http, CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new()
                    .content(format!("Item '{}' not found in collection log.", item_name))
            ))
            .await?;
    }

    Ok(())
} 