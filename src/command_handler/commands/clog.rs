use anyhow::Result;
use serenity::all::{
    CommandInteraction,
    CreateInteractionResponse,
    CreateInteractionResponseMessage,
};
use sqlx::SqlitePool;
use crate::command_handler::{CollectionLogManagerKey, format_points, format_number};
use crate::rank_manager;
use crate::logger;

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

        let item_record = sqlx::query!("SELECT item_id, item_name FROM collection_log_items WHERE item_name = ? GROUP BY item_name ORDER BY percentage", item_name)
        .fetch_one(db)
        .await?;

        // Record the collection log entry
        sqlx::query!(
            "INSERT INTO collection_log_entries (discord_id, item_name, points, item_id) VALUES (?, ?, ?, ?)",
            discord_id,
            item_name,
            points,
            item_record.item_id,
        )
        .execute(db)
        .await?;
        
        // Log the collection log entry
        logger::log_action(
            ctx,
            &discord_id,
            "ADDED CLOG",
            &format!("{} ({} pts)", item_name, format_number(points))
        ).await?;

        // Add points and check for rank up
        let points_update = rank_manager::add_points(
            ctx,
            &discord_id,
            &command.member.as_ref()
                .and_then(|m| Some(m.display_name()))
                .unwrap_or(&command.user.name),
            points,
            db
        ).await?;

        // Format response message
        let message_content = if !points_update.crossed_ranks.is_empty() {
            // User ranked up!
            let rank_text = if points_update.crossed_ranks.len() == 1 {
                format!("the {} rank", points_update.crossed_ranks[0])
            } else {
                let ranks: Vec<_> = points_update.crossed_ranks.iter().map(|r| r.as_str()).collect();
                match ranks.len() {
                    2 => format!("the {} and {} ranks", ranks[0], ranks[1]),
                    _ => {
                        let (last, rest) = ranks.split_last().unwrap();
                        format!("the {}, and {} ranks", rest.join(", "), last)
                    }
                }
            };
            
            let next_rank_info = if let Some((next_rank_points, next_rank_name)) = &points_update.next_rank {
                format!(" Next rank at {} points for {}!", format_number(*next_rank_points), next_rank_name)
            } else {
                "".to_string()
            };
            
            format!(
                "🎆 🎇 **RANK UP!** 🎇 🎆\nCollection log entry recorded: {} (+{} points)! You now have {} and achieved {}!{}",
                item_name,
                format_number(points),
                format_points(points_update.new_points),
                rank_text,
                next_rank_info
            )
        } else if let Some((next_rank_points, next_rank_name)) = points_update.next_rank {
            format!(
                "Collection log entry recorded: {} (+{} points)! You now have {}. Next rank at {} points for {}!",
                item_name,
                format_number(points),
                format_points(points_update.new_points),
                format_number(next_rank_points),
                next_rank_name
            )
        } else {
            format!(
                "Collection log entry recorded: {} (+{} points)! You now have {}!",
                item_name,
                format_number(points),
                format_points(points_update.new_points)
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