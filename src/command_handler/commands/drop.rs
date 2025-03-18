use anyhow::Result;
use serenity::all::{
    CommandInteraction,
    CreateInteractionResponse,
    CreateInteractionResponseMessage,
};
use sqlx::SqlitePool;
use crate::command_handler::{PriceManagerKey, format_gp, format_points, format_number};
use std::sync::Arc;

pub async fn handle_drop(
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

    // Get price manager from context data
    let data = ctx.data.read().await;
    let price_manager = data.get::<PriceManagerKey>()
        .ok_or_else(|| anyhow::anyhow!("Price manager not found"))?;

    // Get item price
    if let Some(value) = price_manager.get_item_price(item_name).await {
        let discord_id = command.user.id.to_string();
        let points = value / 100_000; // 1 point per 100,000 gp

        // Insert or update user
        sqlx::query!(
            "INSERT INTO users (discord_id, points, total_drops) 
             VALUES (?, 0, 0)
             ON CONFLICT(discord_id) DO NOTHING",
            discord_id
        )
        .execute(db)
        .await?;

        // Record the drop
        sqlx::query!(
            "INSERT INTO drops (discord_id, item_name, value) VALUES (?, ?, ?)",
            discord_id,
            item_name,
            value
        )
        .execute(db)
        .await?;

        // Update user points and total drops
        sqlx::query!(
            "UPDATE users 
             SET points = points + ?,
                 total_drops = total_drops + 1
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
                        "Drop recorded: {} ({}) (+{} points)! You now have {}. Next rank at {} points for {}!",
                        item_name,
                        format_gp(value),
                        format_number(points),
                        format_points(user_points.points),
                        format_number(rank.points),
                        rank.role_name
                    )
                }
                None => {
                    format!(
                        "Drop recorded: {} ({}) (+{} points)! You now have {}!",
                        item_name,
                        format_gp(value),
                        format_number(points),
                        format_points(user_points.points)
                    )
                }
            }
        } else {
            format!(
                "Drop recorded: {} ({}) (+{} points)! You now have {}!",
                item_name,
                format_gp(value),
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
                    .content(format!("Item '{}' not found in price database.", item_name))
            ))
            .await?;
    }

    Ok(())
} 