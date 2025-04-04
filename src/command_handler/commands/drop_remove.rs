use anyhow::Result;
use serenity::all::{
    CommandInteraction,
    CreateInteractionResponse,
    CreateInteractionResponseMessage,
};
use sqlx::SqlitePool;
use crate::command_handler::format_gp;
use crate::rank_manager;
use crate::logger;

pub async fn handle_drop_remove(
    command: &CommandInteraction,
    ctx: &serenity::prelude::Context,
    db: &SqlitePool,
) -> Result<()> {
    let discord_id = command.user.id.to_string();
    
    // Get the user's most recent drops (top 10)
    let recent_drops = sqlx::query!(
        "SELECT id, item_name, value, quantity, timestamp 
         FROM drops 
         WHERE discord_id = ? 
         ORDER BY timestamp DESC 
         LIMIT 10",
        discord_id
    )
    .fetch_all(db)
    .await?;
    
    if recent_drops.is_empty() {
        command
            .create_response(&ctx.http, CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new()
                    .content("You don't have any recorded drops to remove.")
            ))
            .await?;
        return Ok(());
    }
    
    // Get the drop ID from the options
    let drop_id = match command.data.options.iter().find(|opt| opt.name == "id") {
        Some(opt) => match opt.value.as_i64() {
            Some(id) => id,
            None => {
                command
                    .create_response(&ctx.http, CreateInteractionResponse::Message(
                        CreateInteractionResponseMessage::new()
                            .content("Invalid drop ID provided.")
                    ))
                    .await?;
                return Ok(());
            }
        },
        None => {
            // If no ID provided, show the list of recent drops
            let mut drops_list = String::from("Your most recent drops:\n");
            
            for drop in &recent_drops {
                let timestamp = drop.timestamp.expect("Timestamp should not be null");
                drops_list.push_str(&format!(
                    "ID {}: {}x {} ({}) - {}\n",
                    drop.id,
                    drop.quantity,
                    drop.item_name,
                    format_gp(drop.value),
                    timestamp.format("%Y-%m-%d %H:%M:%S")
                ));
            }
            
            drops_list.push_str("\nTo remove a drop, use `/drop_remove id:<drop_id>`");
            
            command
                .create_response(&ctx.http, CreateInteractionResponse::Message(
                    CreateInteractionResponseMessage::new()
                        .content(drops_list)
                ))
                .await?;
            return Ok(());
        }
    };
    
    // Find the drop with the given ID
    let drop_to_remove = sqlx::query!(
        "SELECT id, item_name, value, quantity 
         FROM drops 
         WHERE id = ? AND discord_id = ?",
        drop_id,
        discord_id
    )
    .fetch_optional(db)
    .await?;
    
    match drop_to_remove {
        Some(drop) => {
            // Calculate points to deduct
            let points_to_deduct = drop.value / 100_000; // 1 point per 100,000 gp
            
            // Begin transaction
            let mut tx = db.begin().await?;
            
            // Decrease user's total_drops
            sqlx::query!(
                "UPDATE users 
                 SET total_drops = total_drops - ? 
                 WHERE discord_id = ?",
                drop.quantity,
                discord_id
            )
            .execute(&mut *tx)
            .await?;
            
            // Remove the drop
            sqlx::query!(
                "DELETE FROM drops WHERE id = ?",
                drop.id
            )
            .execute(&mut *tx)
            .await?;
            
            // Log the drop removal
            logger::log_action(
                ctx,
                &discord_id,
                "REMOVED DROP",
                &format!("{}x {} ({}) [ID: {}]", drop.quantity, drop.item_name, format_gp(drop.value), drop.id)
            ).await?;
            
            // Commit transaction
            tx.commit().await?;
            
            // Deduct points from user
            if points_to_deduct > 0 {
                let points_update = rank_manager::add_points(
                    ctx,
                    &discord_id,
                    &command.member.as_ref()
                        .and_then(|m| Some(m.display_name()))
                        .unwrap_or(&command.user.name),
                    -points_to_deduct, // Negative to deduct points
                    db
                ).await?;
                
                let message = if !points_update.crossed_ranks.is_empty() {
                    // User ranked down
                    let rank_text = if points_update.crossed_ranks.len() == 1 {
                        format!("You have lost the {} rank.", points_update.crossed_ranks[0])
                    } else {
                        let ranks: Vec<_> = points_update.crossed_ranks.iter().map(|r| r.as_str()).collect();
                        match ranks.len() {
                            2 => format!("You have lost the {} and {} ranks.", ranks[0], ranks[1]),
                            _ => {
                                let (last, rest) = ranks.split_last().unwrap();
                                format!("You have lost the {}, and {} ranks.", rest.join(", "), last)
                            }
                        }
                    };
                    
                    format!(
                        "Drop removed: {}x {} ({}). {} points have been deducted from your total.\n⬇️ **RANK DOWN!** ⬇️\n{}",
                        drop.quantity,
                        drop.item_name,
                        format_gp(drop.value),
                        points_to_deduct,
                        rank_text
                    )
                } else {
                    format!(
                        "Drop removed: {}x {} ({}). {} points have been deducted from your total.",
                        drop.quantity,
                        drop.item_name,
                        format_gp(drop.value),
                        points_to_deduct
                    )
                };
                
                command
                    .create_response(&ctx.http, CreateInteractionResponse::Message(
                        CreateInteractionResponseMessage::new().content(message)
                    ))
                    .await?;
            } else {
                command
                    .create_response(&ctx.http, CreateInteractionResponse::Message(
                        CreateInteractionResponseMessage::new()
                            .content(format!(
                                "Drop removed: {}x {} ({}). No points were deducted.",
                                drop.quantity,
                                drop.item_name,
                                format_gp(drop.value)
                            ))
                    ))
                    .await?;
            }
        },
        None => {
            command
                .create_response(&ctx.http, CreateInteractionResponse::Message(
                    CreateInteractionResponseMessage::new()
                        .content(format!("No drop found with ID {}.", drop_id))
                ))
                .await?;
        }
    }
    
    Ok(())
} 