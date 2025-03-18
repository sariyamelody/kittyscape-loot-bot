use anyhow::Result;
use serenity::all::{
    CommandInteraction,
    CreateInteractionResponse,
    CreateInteractionResponseMessage,
    CreateEmbed,
};
use sqlx::SqlitePool;
use crate::command_handler::{format_points, format_number};

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

            let rank_name = current_rank
                .map(|r| r.role_name)
                .unwrap_or_else(|| "Unranked".to_string());

            let next_rank_name = next_rank
                .map(|r| r.role_name)
                .unwrap_or_else(|| "Maximum".to_string());

            let embed = CreateEmbed::new()
                .title(format!("{}'s Profile", command.member.as_ref().map(|m| m.display_name()).unwrap_or(&command.user.name)))
                .color(0x00ff00)
                .thumbnail(command.user.face())
                .field("Rank", rank_name, true)
                .field("Total Points", format_points(data.points), true)
                .field("Total Drops", format_number(data.total_drops), true)
                .field("Collection Log", format_number(clog_count.into()), true)
                .field(format!("Progress to {}", next_rank_name), progress, false);

            command
                .create_response(&ctx.http, CreateInteractionResponse::Message(
                    CreateInteractionResponseMessage::new()
                        .embed(embed)
                ))
                .await?;
        }
        None => {
            let embed = CreateEmbed::new()
                .title(format!("{}'s Profile", command.member.as_ref().map(|m| m.display_name()).unwrap_or(&command.user.name)))
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