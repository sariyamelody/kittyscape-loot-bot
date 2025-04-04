use anyhow::Result;
use serenity::all::{
    CommandInteraction,
    CreateInteractionResponse,
    CreateInteractionResponseMessage,
};
use sqlx::SqlitePool;
use crate::command_handler::{PriceManagerKey, format_gp, format_points, format_number};
use crate::rank_manager;
use crate::logger;

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

    let quantity = options
        .iter()
        .find(|opt| opt.name == "quantity")
        .and_then(|opt| opt.value.as_i64())
        .unwrap_or(1);

    // Get price manager from context data
    let data = ctx.data.read().await;
    let price_manager = data.get::<PriceManagerKey>()
        .ok_or_else(|| anyhow::anyhow!("Price manager not found"))?;

    // Get item price
    if let Some(value) = price_manager.get_item_price(item_name).await {
        let discord_id = command.user.id.to_string();
        let total_value = value * quantity;
        let points = total_value / 100_000; // 1 point per 100,000 gp

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
            "INSERT INTO drops (discord_id, item_name, value, quantity) VALUES (?, ?, ?, ?)",
            discord_id,
            item_name,
            total_value,
            quantity
        )
        .execute(db)
        .await?;

        // Log the drop
        logger::log_action(
            ctx,
            &discord_id,
            "ADDED DROP",
            &format!("{}x {} ({})", quantity, item_name, format_gp(total_value))
        ).await?;

        // Update total drops
        sqlx::query!(
            "UPDATE users 
             SET total_drops = total_drops + ?
             WHERE discord_id = ?",
            quantity,
            discord_id
        )
        .execute(db)
        .await?;

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
                "🎆 🎇 **RANK UP!** 🎇 🎆\nDrop recorded: {}x {} ({}) (+{} points)! You now have {} and achieved {}!{}",
                format_number(quantity),
                item_name,
                format_gp(total_value),
                format_number(points),
                format_points(points_update.new_points),
                rank_text,
                next_rank_info
            )
        } else if let Some((next_rank_points, next_rank_name)) = points_update.next_rank {
            format!(
                "Drop recorded: {}x {} ({}) (+{} points)! You now have {}. Next rank at {} points for {}!",
                format_number(quantity),
                item_name,
                format_gp(total_value),
                format_number(points),
                format_points(points_update.new_points),
                format_number(next_rank_points),
                next_rank_name
            )
        } else {
            format!(
                "Drop recorded: {}x {} ({}) (+{} points)! You now have {}!",
                format_number(quantity),
                item_name,
                format_gp(total_value),
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
                    .content(format!("Item '{}' not found in price database.", item_name))
            ))
            .await?;
    }

    Ok(())
} 