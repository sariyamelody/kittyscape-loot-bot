use anyhow::Result;
use serenity::all::{
    CommandInteraction,
    CreateInteractionResponse,
    CreateInteractionResponseMessage,
};
use sqlx::SqlitePool;
use crate::rank_manager;
use crate::logger;

pub async fn handle_clog_remove(
    command: &CommandInteraction,
    ctx: &serenity::prelude::Context,
    db: &SqlitePool,
) -> Result<()> {
    let discord_id = command.user.id.to_string();
    
    // Get the user's most recent collection log entries (top 10)
    let recent_entries = sqlx::query!(
        "SELECT id, item_name, points, timestamp 
         FROM collection_log_entries 
         WHERE discord_id = ? 
         ORDER BY timestamp DESC 
         LIMIT 10",
        discord_id
    )
    .fetch_all(db)
    .await?;
    
    if recent_entries.is_empty() {
        command
            .create_response(&ctx.http, CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new()
                    .content("You don't have any recorded collection log entries to remove.")
            ))
            .await?;
        return Ok(());
    }
    
    // Get the entry ID from the options
    let entry_id = match command.data.options.iter().find(|opt| opt.name == "id") {
        Some(opt) => match opt.value.as_i64() {
            Some(id) => id,
            None => {
                command
                    .create_response(&ctx.http, CreateInteractionResponse::Message(
                        CreateInteractionResponseMessage::new()
                            .content("Invalid collection log entry ID provided.")
                    ))
                    .await?;
                return Ok(());
            }
        },
        None => {
            // If no ID provided, show the list of recent collection log entries
            let mut entries_list = String::from("Your most recent collection log entries:\n");
            
            for entry in &recent_entries {
                let timestamp = entry.timestamp.expect("Timestamp should not be null");
                entries_list.push_str(&format!(
                    "ID {}: {} ({} pts) - {}\n",
                    entry.id,
                    entry.item_name,
                    entry.points,
                    timestamp.format("%Y-%m-%d %H:%M:%S")
                ));
            }
            
            entries_list.push_str("\nTo remove an entry, use `/clog_remove id:<entry_id>`");
            
            command
                .create_response(&ctx.http, CreateInteractionResponse::Message(
                    CreateInteractionResponseMessage::new()
                        .content(entries_list)
                ))
                .await?;
            return Ok(());
        }
    };
    
    // Find the collection log entry with the given ID
    let entry_to_remove = sqlx::query!(
        "SELECT id, item_name, points 
         FROM collection_log_entries 
         WHERE id = ? AND discord_id = ?",
        entry_id,
        discord_id
    )
    .fetch_optional(db)
    .await?;
    
    match entry_to_remove {
        Some(entry) => {
            // Begin transaction
            let mut tx = db.begin().await?;
            
            // Remove the collection log entry
            sqlx::query!(
                "DELETE FROM collection_log_entries WHERE id = ?",
                entry.id
            )
            .execute(&mut *tx)
            .await?;
            
            // Log the collection log entry removal
            logger::log_action(
                ctx,
                &discord_id,
                "REMOVED CLOG",
                &format!("{} ({} pts) [ID: {}]", entry.item_name, entry.points, entry.id)
            ).await?;
            
            // Commit transaction
            tx.commit().await?;
            
            // Deduct points from user
            if entry.points > 0 {
                let points_update = rank_manager::add_points(
                    ctx,
                    &discord_id,
                    &command.member.as_ref()
                        .and_then(|m| Some(m.display_name()))
                        .unwrap_or(&command.user.name),
                    -entry.points, // Negative to deduct points
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
                        "Collection log entry removed: {} ({} pts). Points have been deducted from your total.\n⬇️ **RANK DOWN!** ⬇️\n{}",
                        entry.item_name,
                        entry.points,
                        rank_text
                    )
                } else {
                    format!(
                        "Collection log entry removed: {} ({} pts). Points have been deducted from your total.",
                        entry.item_name,
                        entry.points
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
                                "Collection log entry removed: {} (0 pts). No points were deducted.",
                                entry.item_name
                            ))
                    ))
                    .await?;
            }
        },
        None => {
            command
                .create_response(&ctx.http, CreateInteractionResponse::Message(
                    CreateInteractionResponseMessage::new()
                        .content(format!("No collection log entry found with ID {}.", entry_id))
                ))
                .await?;
        }
    }
    
    Ok(())
} 