use anyhow::Result;
use serenity::prelude::*;
use sqlx::SqlitePool;
use crate::config::ConfigKey;
use crate::command_handler::format_points;

pub struct PointsUpdate {
    pub old_points: i64,
    pub new_points: i64,
    pub next_rank: Option<(i64, String)>,
}

pub async fn add_points(
    ctx: &Context,
    discord_id: &str,
    user_name: &str,
    points_to_add: i64,
    db: &SqlitePool,
) -> Result<PointsUpdate> {
    // Insert or update user
    sqlx::query!(
        "INSERT INTO users (discord_id, points, total_drops) 
         VALUES (?, 0, 0)
         ON CONFLICT(discord_id) DO NOTHING",
        discord_id
    )
    .execute(db)
    .await?;

    // Get current points before update
    let old_points = sqlx::query!(
        "SELECT points FROM users WHERE discord_id = ?",
        discord_id
    )
    .fetch_one(db)
    .await?
    .points;

    // Update user points
    sqlx::query!(
        "UPDATE users 
         SET points = points + ?
         WHERE discord_id = ?",
        points_to_add,
        discord_id
    )
    .execute(db)
    .await?;

    // Get new points total
    let new_points = old_points + points_to_add;

    // Check if any rank thresholds were crossed
    let crossed_ranks = sqlx::query!(
        "SELECT points, role_name FROM rank_thresholds 
         WHERE points > ? AND points <= ?
         ORDER BY points DESC",
        old_points,
        new_points
    )
    .fetch_all(db)
    .await?;

    // Send notifications for any crossed rank thresholds
    if !crossed_ranks.is_empty() {
        let data = ctx.data.read().await;
        if let Some(config) = data.get::<ConfigKey>() {
            for rank in crossed_ranks {
                let notification = format!(
                    "🎉 **Rank Up Alert!**\n{} has reached {} points and is ready for the {} role!",
                    user_name,
                    format_points(new_points),
                    rank.role_name
                );

                if let Err(why) = config.mod_channel_id
                    .say(&ctx.http, notification)
                    .await
                {
                    tracing::error!("Failed to send rank up notification: {:?}", why);
                }
            }
        }
    }

    // Get next rank for progress message
    let next_rank = sqlx::query!(
        "SELECT points, role_name FROM rank_thresholds 
         WHERE points > ? 
         ORDER BY points ASC 
         LIMIT 1",
        new_points
    )
    .fetch_optional(db)
    .await?
    .map(|rank| (rank.points, rank.role_name));

    Ok(PointsUpdate {
        old_points,
        new_points,
        next_rank,
    })
}

pub async fn get_rank_progress(
    discord_id: &str,
    db: &SqlitePool,
) -> Result<Option<(i64, String)>> {
    let user_points = sqlx::query!(
        "SELECT points FROM users WHERE discord_id = ?",
        discord_id
    )
    .fetch_one(db)
    .await?;

    // Get the next rank threshold
    let next_rank = sqlx::query!(
        "SELECT points, role_name FROM rank_thresholds 
         WHERE points > ? 
         ORDER BY points ASC 
         LIMIT 1",
        user_points.points
    )
    .fetch_optional(db)
    .await?;

    Ok(next_rank.map(|rank| (rank.points, rank.role_name)))
} 