use anyhow::Result;
use serenity::all::{
    CommandInteraction,
    CreateInteractionResponse,
    CreateInteractionResponseMessage,
    CreateEmbed,
    UserId,
};
use sqlx::SqlitePool;
use crate::command_handler::{format_points, format_number, format_gp};

pub async fn handle_leaderboard(
    command: &CommandInteraction,
    ctx: &serenity::prelude::Context,
    db: &SqlitePool,
) -> Result<()> {
    // Get all-time top users
    let top_users = sqlx::query!(
        r#"WITH user_clogs AS (
            SELECT discord_id, COUNT(*) as count
            FROM collection_log_entries
            GROUP BY discord_id
        )
        SELECT u.discord_id, u.points, u.total_drops, COALESCE(c.count, 0) as clog_count
        FROM users u
        LEFT JOIN user_clogs c ON u.discord_id = c.discord_id
        ORDER BY u.points DESC
        LIMIT 10"#
    )
    .fetch_all(db)
    .await?;

    // Get top droppers for the last 30 days with their most valuable drop
    let top_droppers = sqlx::query!(
        r#"WITH monthly_drops AS (
            SELECT discord_id,
                   COUNT(*) as drop_count,
                   SUM(value) as total_value,
                   MAX(value) as best_drop_value
            FROM drops
            WHERE timestamp >= datetime('now', '-30 days')
            GROUP BY discord_id
            ORDER BY total_value DESC
            LIMIT 5
        ),
        best_drops AS (
            SELECT d1.discord_id, d1.item_name as best_drop_name
            FROM drops d1
            JOIN monthly_drops m ON d1.discord_id = m.discord_id
            WHERE d1.timestamp >= datetime('now', '-30 days')
            AND d1.value = (
                SELECT MAX(value)
                FROM drops d2
                WHERE d2.discord_id = d1.discord_id
                AND d2.timestamp >= datetime('now', '-30 days')
            )
            GROUP BY d1.discord_id
        )
        SELECT m.discord_id, m.drop_count, m.total_value, m.best_drop_value, b.best_drop_name, u.points
        FROM monthly_drops m
        LEFT JOIN best_drops b ON m.discord_id = b.discord_id
        JOIN users u ON m.discord_id = u.discord_id"#
    )
    .fetch_all(db)
    .await?;

    // Get top collection loggers for the last 30 days with their most valuable entry
    let top_cloggers = sqlx::query!(
        r#"WITH monthly_clogs AS (
            SELECT discord_id,
                   COUNT(*) as entry_count,
                   SUM(points) as total_points,
                   MAX(points) as best_entry_points
            FROM collection_log_entries
            WHERE timestamp >= datetime('now', '-30 days')
            GROUP BY discord_id
            ORDER BY entry_count DESC
            LIMIT 5
        ),
        best_entries AS (
            SELECT c1.discord_id, c1.item_name as best_entry_name
            FROM collection_log_entries c1
            JOIN monthly_clogs m ON c1.discord_id = m.discord_id
            WHERE c1.timestamp >= datetime('now', '-30 days')
            AND c1.points = (
                SELECT MAX(points)
                FROM collection_log_entries c2
                WHERE c2.discord_id = c1.discord_id
                AND c2.timestamp >= datetime('now', '-30 days')
            )
            GROUP BY c1.discord_id
        )
        SELECT m.discord_id, m.entry_count, m.total_points, m.best_entry_points, b.best_entry_name, u.points
        FROM monthly_clogs m
        LEFT JOIN best_entries b ON m.discord_id = b.discord_id
        JOIN users u ON m.discord_id = u.discord_id"#
    )
    .fetch_all(db)
    .await?;

    // Format all-time leaderboard
    let mut all_time = String::new();
    for (i, user) in top_users.iter().enumerate() {
        let discord_id = user.discord_id.clone().expect("Missing discord ID");
        let user_id = UserId::new(discord_id.parse::<u64>().expect("Invalid discord ID"));
        let user_name = ctx.http.get_user(user_id).await?.name;
        all_time.push_str(&format!(
            "{}. **{}**\n• Points: {}\n• Total Drops: {}\n• Collection Log: {}\n\n",
            i + 1,
            user_name,
            format_points(user.points),
            format_number(user.total_drops),
            format_number(user.clog_count.into())
        ));
    }

    // Format monthly droppers
    let mut monthly_drops = String::new();
    for (i, user) in top_droppers.iter().enumerate() {
        let discord_id = user.discord_id.clone().expect("Missing discord ID");
        let user_id = UserId::new(discord_id.parse::<u64>().expect("Invalid discord ID"));
        let user_name = ctx.http.get_user(user_id).await?.name;
        
        let best_drop = match &user.best_drop_name {
            Some(name) => format!("\n• Best Drop: {} ({})", name, format_gp(user.best_drop_value.unwrap_or(0) as i64)),
            None => String::new()
        };
        
        monthly_drops.push_str(&format!(
            "{}. **{}**\n• Drops: {}\n• Total Value: {}{}",
            i + 1,
            user_name,
            format_number(user.drop_count.unwrap_or(0) as i64),
            format_gp(user.total_value.unwrap_or(0) as i64),
            best_drop
        ));
        
        if i < top_droppers.len() - 1 {
            monthly_drops.push_str("\n\n");
        }
    }
    if monthly_drops.is_empty() {
        monthly_drops = "No drops recorded in the past 30 days".to_string();
    }

    // Format monthly collection loggers
    let mut monthly_clogs = String::new();
    for (i, user) in top_cloggers.iter().enumerate() {
        let discord_id = user.discord_id.clone().expect("Missing discord ID");
        let user_id = UserId::new(discord_id.parse::<u64>().expect("Invalid discord ID"));
        let user_name = ctx.http.get_user(user_id).await?.name;
        
        let best_entry = match &user.best_entry_name {
            Some(name) => format!("\n• Best Entry: {} ({})", name, format_points(user.best_entry_points.unwrap_or(0).into())),
            None => String::new()
        };
        
        monthly_clogs.push_str(&format!(
            "{}. **{}**\n• Entries: {}\n• Points: {}{}",
            i + 1,
            user_name,
            format_number(user.entry_count.unwrap_or(0).into()),
            format_points(user.total_points.unwrap_or(0).into()),
            best_entry
        ));
        
        if i < top_cloggers.len() - 1 {
            monthly_clogs.push_str("\n\n");
        }
    }
    if monthly_clogs.is_empty() {
        monthly_clogs = "No collection log entries in the past 30 days".to_string();
    }

    let embed = CreateEmbed::new()
        .title("🏆 Leaderboards")
        .field("All-Time Top 10", all_time, false)
        .field("📅 Top Droppers (30 Days)", monthly_drops, true)
        .field("📅 Top Collection Loggers (30 Days)", monthly_clogs, true)
        .color(0xffd700);

    command
        .create_response(&ctx.http, CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new()
                .embed(embed)
        ))
        .await?;

    Ok(())
} 