use anyhow::Result;
use serenity::all::{
    CommandInteraction,
    CreateInteractionResponse,
    CreateInteractionResponseMessage,
    CreateEmbed,
};
use sqlx::SqlitePool;
use crate::command_handler::{format_points, format_number, format_gp};

pub async fn handle_stats(
    command: &CommandInteraction,
    ctx: &serenity::prelude::Context,
    db: &SqlitePool,
) -> Result<()> {
    let discord_id = command.user.id.to_string();
    
    // Get user data
    let user_data = sqlx::query!(
        "SELECT points, total_drops FROM users WHERE discord_id = ?",
        discord_id
    )
    .fetch_optional(db)
    .await?;

    match user_data {
        Some(data) => {
            // Get collection log count
            let clog_count = sqlx::query!(
                "SELECT COUNT(*) as count FROM collection_log_entries WHERE discord_id = ?",
                discord_id
            )
            .fetch_one(db)
            .await?
            .count;

            // Get current rank
            let current_rank = sqlx::query!(
                "SELECT role_name, points 
                 FROM rank_thresholds 
                 WHERE points <= ? 
                 ORDER BY points DESC 
                 LIMIT 1",
                data.points
            )
            .fetch_optional(db)
            .await?;

            // Get next rank
            let next_rank = sqlx::query!(
                "SELECT role_name, points 
                 FROM rank_thresholds 
                 WHERE points > ? 
                 ORDER BY points ASC 
                 LIMIT 1",
                data.points
            )
            .fetch_optional(db)
            .await?;

            // Calculate progress to next rank
            let progress = if let Some(ref next) = next_rank {
                let current = current_rank.as_ref().map(|r| r.points).unwrap_or(0);
                let progress = data.points - current;
                let needed = next.points - current;
                let percentage = (progress as f64 / needed as f64 * 100.0).round();
                format!(
                    "{:.1}% ({} / {})",
                    percentage,
                    format_number(progress),
                    format_points(needed)
                )
            } else {
                "Maximum rank achieved!".to_string()
            };

            // Get 5 most recent drops
            let recent_drops = sqlx::query!(
                "SELECT item_name, quantity, value, timestamp 
                 FROM drops 
                 WHERE discord_id = ? 
                 ORDER BY timestamp DESC 
                 LIMIT 5",
                discord_id
            )
            .fetch_all(db)
            .await?;

            // Get 5 most recent collection log entries
            let recent_clogs = sqlx::query!(
                "SELECT item_name, points, timestamp 
                 FROM collection_log_entries 
                 WHERE discord_id = ? 
                 ORDER BY timestamp DESC 
                 LIMIT 5",
                discord_id
            )
            .fetch_all(db)
            .await?;

            // Get most valuable drop
            let most_valuable_drop = sqlx::query!(
                "SELECT item_name, quantity, value 
                 FROM drops 
                 WHERE discord_id = ? 
                 ORDER BY value DESC 
                 LIMIT 1",
                discord_id
            )
            .fetch_optional(db)
            .await?;

            // Get rarest collection log entry
            let rarest_clog = sqlx::query!(
                "SELECT item_name, points 
                 FROM collection_log_entries 
                 WHERE discord_id = ? 
                 ORDER BY points DESC 
                 LIMIT 1",
                discord_id
            )
            .fetch_optional(db)
            .await?;

            // Calculate average drop value
            let avg_drop_value = sqlx::query!(
                "SELECT AVG(CAST(value AS FLOAT)) as avg_value 
                 FROM drops 
                 WHERE discord_id = ?",
                discord_id
            )
            .fetch_one(db)
            .await?
            .avg_value
            .unwrap_or(0.0) as i64;

            // Calculate average collection log points
            let avg_clog_points = sqlx::query!(
                "SELECT AVG(CAST(points AS FLOAT)) as avg_points 
                 FROM collection_log_entries 
                 WHERE discord_id = ?",
                discord_id
            )
            .fetch_one(db)
            .await?
            .avg_points
            .unwrap_or(0.0) as i64;

            let rank_name = current_rank
                .map(|r| r.role_name)
                .unwrap_or_else(|| "Unranked".to_string());

            let next_rank_name = next_rank
                .map(|r| r.role_name)
                .unwrap_or_else(|| "Maximum".to_string());

            // Format recent drops
            let recent_drops_text = if recent_drops.is_empty() {
                "No drops recorded yet".to_string()
            } else {
                recent_drops
                    .iter()
                    .map(|drop| {
                        format!(
                            "• {}x {} ({})",
                            format_number(drop.quantity),
                            drop.item_name,
                            format_gp(drop.value)
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            };

            // Format recent collection log entries
            let recent_clogs_text = if recent_clogs.is_empty() {
                "No collection log entries yet".to_string()
            } else {
                recent_clogs
                    .iter()
                    .map(|clog| {
                        format!(
                            "• {} (+{} pts)",
                            clog.item_name,
                            format_number(clog.points)
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            };

            // Format most valuable drop
            let most_valuable_text = most_valuable_drop
                .map(|drop| {
                    format!(
                        "{}x {} ({})",
                        format_number(drop.quantity),
                        drop.item_name,
                        format_gp(drop.value)
                    )
                })
                .unwrap_or_else(|| "No drops recorded yet".to_string());

            // Format rarest collection log entry
            let rarest_clog_text = rarest_clog
                .map(|clog| {
                    format!(
                        "{} ({} pts)",
                        clog.item_name,
                        format_number(clog.points)
                    )
                })
                .unwrap_or_else(|| "No collection log entries yet".to_string());

            let embed = CreateEmbed::new()
                .title(format!("{}'s Profile", command.member.as_ref()
                    .and_then(|m| Some(m.display_name()))
                    .unwrap_or(&command.user.name)))
                .color(0x00ff00)
                .thumbnail(command.user.face())
                .field("Rank", rank_name, true)
                .field("Total Points", format_points(data.points), true)
                .field("Total Drops", format_number(data.total_drops), true)
                .field("Collection Log", format_number(clog_count.into()), true)
                .field("Average Drop Value", format_gp(avg_drop_value), true)
                .field("Average CLog Points", format_points(avg_clog_points), true)
                .field(format!("Progress to {}", next_rank_name), progress, false)
                .field("Recent Drops", recent_drops_text, false)
                .field("Recent Collection Log", recent_clogs_text, false)
                .field("Most Valuable Drop", most_valuable_text, true)
                .field("Rarest Collection Log Entry", rarest_clog_text, true);

            command
                .create_response(&ctx.http, CreateInteractionResponse::Message(
                    CreateInteractionResponseMessage::new()
                        .embed(embed)
                ))
                .await?;
        }
        None => {
            let embed = CreateEmbed::new()
                .title(format!("{}'s Profile", command.member.as_ref()
                    .and_then(|m| Some(m.display_name()))
                    .unwrap_or(&command.user.name)))
                .color(0xff0000)
                .thumbnail(command.user.face())
                .description("No stats recorded yet! Start using /drop or /clog to track your progress.");

            command
                .create_response(&ctx.http, CreateInteractionResponse::Message(
                    CreateInteractionResponseMessage::new()
                        .embed(embed)
                ))
                .await?;
        }
    }

    Ok(())
} 