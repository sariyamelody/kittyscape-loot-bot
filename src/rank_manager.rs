use anyhow::Result;
use serenity::prelude::*;
use serenity::model::prelude::*;
use serenity::utils::MessageBuilder;
use sqlx::SqlitePool;
use crate::config::ConfigKey;
use crate::command_handler::format_points;

pub struct PointsUpdate {
    pub old_points: i64,
    pub new_points: i64,
    pub next_rank: Option<(i64, String)>,
    pub crossed_ranks: Vec<String>,
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

    // Track rank changes
    let mut crossed_ranks = Vec::new();
    
    // If adding points, check if any rank thresholds were crossed (ranked up)
    if points_to_add > 0 {
        let ranks_crossed = sqlx::query!(
            "SELECT points, role_name FROM rank_thresholds 
             WHERE points > ? AND points <= ?
             ORDER BY points ASC",
            old_points,
            new_points
        )
        .fetch_all(db)
        .await?;
        
        // Extract crossed rank names
        crossed_ranks = ranks_crossed.iter()
            .map(|r| r.role_name.clone())
            .collect();
            
        // Send notification if ranks were crossed
        if !crossed_ranks.is_empty() {
            let data = ctx.data.read().await;
            if let Some(config) = data.get::<ConfigKey>() {
                // Format ranks list
                let ranks_text = if ranks_crossed.len() == 1 {
                    ranks_crossed[0].role_name.clone()
                } else {
                    let ranks: Vec<_> = ranks_crossed.iter().map(|r| r.role_name.as_str()).collect();
                    match ranks.len() {
                        2 => format!("{} and {}", ranks[0], ranks[1]),
                        _ => {
                            let (last, rest) = ranks.split_last().unwrap();
                            format!("{}, and {}", rest.join(", "), last)
                        }
                    }
                };

                // Create notification with all crossed ranks
                let notification = if ranks_crossed.len() == 1 {
                    format!(
                        "🎉 **Rank Up Alert!**\n{} has reached {} and is ready for the {} role!",
                        user_name,
                        format_points(new_points),
                        ranks_text
                    )
                } else {
                    format!(
                        "🎉 **Multiple Rank Up Alert!**\n{} has reached {} and is ready for the following roles: {}!",
                        user_name,
                        format_points(new_points),
                        ranks_text
                    )
                };

                let mut rank_up_message = MessageBuilder::new();

                rank_up_message.push(notification);

                let discord_user_id = UserId::new(discord_id.parse().unwrap());

                if let Some(rank_request_channel_id) = config.rank_request_channel_id {
                    //If rank request channel exists and we hit the threshold rank this level up
                    if ranks_text.contains("Meowster") { //This should technically be a configurable variable but that can come later
                        rank_up_message.push("\n")
                        .mention(&discord_user_id)
                        .push(", you may now select any rank icon you want in ")
                        .mention(&rank_request_channel_id)
                        .push("!");
                    }
                };

                if let Err(why) = config.mod_channel_id
                    .say(&ctx.http, rank_up_message.build())
                    .await
                {
                    tracing::error!("Failed to send rank up notification: {:?}", why);
                }
            }
        }
    } 
    // If removing points, check if any rank thresholds were crossed (ranked down)
    else if points_to_add < 0 {
        let ranks_lost = sqlx::query!(
            "SELECT points, role_name FROM rank_thresholds 
             WHERE points > ? AND points <= ?
             ORDER BY points DESC",
            new_points,
            old_points
        )
        .fetch_all(db)
        .await?;
        
        // Extract lost rank names
        crossed_ranks = ranks_lost.iter()
            .map(|r| r.role_name.clone())
            .collect();
            
        // Send notification if ranks were lost
        if !crossed_ranks.is_empty() {
            let data = ctx.data.read().await;
            if let Some(config) = data.get::<ConfigKey>() {
                // Format ranks list
                let ranks_text = if ranks_lost.len() == 1 {
                    ranks_lost[0].role_name.clone()
                } else {
                    let ranks: Vec<_> = ranks_lost.iter().map(|r| r.role_name.as_str()).collect();
                    match ranks.len() {
                        2 => format!("{} and {}", ranks[0], ranks[1]),
                        _ => {
                            let (last, rest) = ranks.split_last().unwrap();
                            format!("{}, and {}", rest.join(", "), last)
                        }
                    }
                };

                // Create notification about lost ranks
                let notification = if ranks_lost.len() == 1 {
                    format!(
                        "⬇️ **Rank Down Alert!**\n{} is now at {} and has lost the {} role.",
                        user_name,
                        format_points(new_points),
                        ranks_text
                    )
                } else {
                    format!(
                        "⬇️ **Multiple Rank Down Alert!**\n{} is now at {} and has lost the following roles: {}.",
                        user_name,
                        format_points(new_points),
                        ranks_text
                    )
                };

                if let Err(why) = config.mod_channel_id
                    .say(&ctx.http, notification)
                    .await
                {
                    tracing::error!("Failed to send rank down notification: {:?}", why);
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
        crossed_ranks,
    })
}

pub async fn notify_rank_transition(
    ctx: &Context,
    discord_id: &str,
    user_name: &str,
    old_points: i64,
    new_points: i64,
    db: &SqlitePool,
) -> Result<()> {
    if new_points > old_points {
        let ranks_crossed = sqlx::query_as::<_, (i64, String)>(
            "SELECT points, role_name FROM rank_thresholds
             WHERE points > ? AND points <= ?
             ORDER BY points ASC"
        )
        .bind(old_points)
        .bind(new_points)
        .fetch_all(db)
        .await?;

        if !ranks_crossed.is_empty() {
            let data = ctx.data.read().await;
            if let Some(config) = data.get::<ConfigKey>() {
                let ranks_text = if ranks_crossed.len() == 1 {
                    ranks_crossed[0].1.clone()
                } else {
                    let ranks: Vec<_> = ranks_crossed.iter().map(|r| r.1.as_str()).collect();
                    match ranks.len() {
                        2 => format!("{} and {}", ranks[0], ranks[1]),
                        _ => {
                            let (last, rest) = ranks.split_last().unwrap();
                            format!("{}, and {}", rest.join(", "), last)
                        }
                    }
                };

                let notification = if ranks_crossed.len() == 1 {
                    format!(
                        "🎉 **Rank Up Alert!**\n{} has reached {} and is ready for the {} role!",
                        user_name,
                        format_points(new_points),
                        ranks_text
                    )
                } else {
                    format!(
                        "🎉 **Multiple Rank Up Alert!**\n{} has reached {} and is ready for the following roles: {}!",
                        user_name,
                        format_points(new_points),
                        ranks_text
                    )
                };

                let mut rank_up_message = MessageBuilder::new();
                rank_up_message.push(notification);

                let discord_user_id = UserId::new(discord_id.parse().unwrap());
                if let Some(rank_request_channel_id) = config.rank_request_channel_id {
                    if ranks_text.contains("Meowster") {
                        rank_up_message
                            .push("\n")
                            .mention(&discord_user_id)
                            .push(", you may now select any rank icon you want in ")
                            .mention(&rank_request_channel_id)
                            .push("!");
                    }
                };

                if let Err(why) = config.mod_channel_id.say(&ctx.http, rank_up_message.build()).await {
                    tracing::error!("Failed to send rank up notification: {:?}", why);
                }
            }
        }
    } else if new_points < old_points {
        let ranks_lost = sqlx::query_as::<_, (i64, String)>(
            "SELECT points, role_name FROM rank_thresholds
             WHERE points > ? AND points <= ?
             ORDER BY points DESC"
        )
        .bind(new_points)
        .bind(old_points)
        .fetch_all(db)
        .await?;

        if !ranks_lost.is_empty() {
            let data = ctx.data.read().await;
            if let Some(config) = data.get::<ConfigKey>() {
                let ranks_text = if ranks_lost.len() == 1 {
                    ranks_lost[0].1.clone()
                } else {
                    let ranks: Vec<_> = ranks_lost.iter().map(|r| r.1.as_str()).collect();
                    match ranks.len() {
                        2 => format!("{} and {}", ranks[0], ranks[1]),
                        _ => {
                            let (last, rest) = ranks.split_last().unwrap();
                            format!("{}, and {}", rest.join(", "), last)
                        }
                    }
                };

                let notification = if ranks_lost.len() == 1 {
                    format!(
                        "⬇️ **Rank Down Alert!**\n{} is now at {} and has lost the {} role.",
                        user_name,
                        format_points(new_points),
                        ranks_text
                    )
                } else {
                    format!(
                        "⬇️ **Multiple Rank Down Alert!**\n{} is now at {} and has lost the following roles: {}.",
                        user_name,
                        format_points(new_points),
                        ranks_text
                    )
                };

                if let Err(why) = config.mod_channel_id.say(&ctx.http, notification).await {
                    tracing::error!("Failed to send rank down notification: {:?}", why);
                }
            }
        }
    }

    Ok(())
}