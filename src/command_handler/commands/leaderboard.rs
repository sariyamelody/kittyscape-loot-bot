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
                   MAX(value) as best_drop_value,
                   (SELECT item_name 
                    FROM drops d2 
                    WHERE d2.discord_id = d1.discord_id 
                    AND d2.timestamp >= datetime('now', '-30 days')
                    AND d2.value = (
                        SELECT MAX(value) 
                        FROM drops d3 
                        WHERE d3.discord_id = d1.discord_id 
                        AND d3.timestamp >= datetime('now', '-30 days')
                    )
                    LIMIT 1) as best_drop_name
            FROM drops d1
            WHERE timestamp >= datetime('now', '-30 days')
            GROUP BY discord_id
            ORDER BY total_value DESC
            LIMIT 5
        )
        SELECT d.*, u.points
        FROM monthly_drops d
        JOIN users u ON d.discord_id = u.discord_id"#
    )
    .fetch_all(db)
    .await?;

    // Get top collection loggers for the last 30 days with their most valuable entry
    let top_cloggers = sqlx::query!(
        r#"WITH monthly_clogs AS (
            SELECT discord_id,
                   COUNT(*) as entry_count,
                   SUM(points) as total_points,
                   MAX(points) as best_entry_points,
                   (SELECT item_name 
                    FROM collection_log_entries c2 
                    WHERE c2.discord_id = c1.discord_id 
                    AND c2.timestamp >= datetime('now', '-30 days')
                    AND c2.points = (
                        SELECT MAX(points) 
                        FROM collection_log_entries c3 
                        WHERE c3.discord_id = c1.discord_id 
                        AND c3.timestamp >= datetime('now', '-30 days')
                    )
                    LIMIT 1) as best_entry_name
            FROM collection_log_entries c1
            WHERE timestamp >= datetime('now', '-30 days')
            GROUP BY discord_id
            ORDER BY entry_count DESC
            LIMIT 5
        )
        SELECT c.*, u.points
        FROM monthly_clogs c
        JOIN users u ON c.discord_id = u.discord_id"#
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
        let discord_id = user.discord_id.clone();
        let user_id = UserId::new(discord_id.parse::<u64>().expect("Invalid discord ID"));
        let user_name = ctx.http.get_user(user_id).await?.name;
        let best_drop = if let Some(name) = &user.best_drop_name {
            format!("\n• Best Drop: {} ({})", name, format_gp(user.best_drop_value.into()))
        } else {
            String::new()
        };
        monthly_drops.push_str(&format!(
            "{}. **{}**\n• Drops: {}\n• Total Value: {}{}\n",
            i + 1,
            user_name,
            format_number(user.drop_count.into()),
            format_gp(user.total_value.into()),
            best_drop
        ));
    }
    if monthly_drops.is_empty() {
        monthly_drops = "No drops recorded this month".to_string();
    }

    // Format monthly collection loggers
    let mut monthly_clogs = String::new();
    for (i, user) in top_cloggers.iter().enumerate() {
        let discord_id = user.discord_id.clone();
        let user_id = UserId::new(discord_id.parse::<u64>().expect("Invalid discord ID"));
        let user_name = ctx.http.get_user(user_id).await?.name;
        let best_entry = if let Some(name) = &user.best_entry_name {
            format!("\n• Best Entry: {} ({})", name, format_points(user.best_entry_points.into()))
        } else {
            String::new()
        };
        monthly_clogs.push_str(&format!(
            "{}. **{}**\n• Entries: {}\n• Points: {}{}\n",
            i + 1,
            user_name,
            format_number(user.entry_count.into()),
            format_points(user.total_points.into()),
            best_entry
        ));
    }
    if monthly_clogs.is_empty() {
        monthly_clogs = "No collection log entries this month".to_string();
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