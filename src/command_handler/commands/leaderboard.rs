use anyhow::Result;
use serenity::all::{
    CommandInteraction,
    CreateInteractionResponse,
    CreateInteractionResponseMessage,
    CreateEmbed,
    UserId,
};
use sqlx::SqlitePool;
use crate::command_handler::{format_points, format_number};

pub async fn handle_leaderboard(
    command: &CommandInteraction,
    ctx: &serenity::prelude::Context,
    db: &SqlitePool,
) -> Result<()> {
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

    let mut description = String::new();
    for (i, user) in top_users.iter().enumerate() {
        let discord_id = user.discord_id.clone().expect("Missing discord ID");
        let user_id = UserId::new(discord_id.parse::<u64>().expect("Invalid discord ID"));
        let user_name = ctx.http.get_user(user_id).await?.name;
        description.push_str(&format!(
            "{}. **{}**\n• Points: {}\n• Total Drops: {}\n• Collection Log: {}\n\n",
            i + 1,
            user_name,
            format_points(user.points),
            format_number(user.total_drops),
            format_number(user.clog_count.into())
        ));
    }

    let embed = CreateEmbed::new()
        .title("🏆 Top 10 Looters")
        .description(description)
        .color(0xffd700);

    command
        .create_response(&ctx.http, CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new()
                .embed(embed)
        ))
        .await?;

    Ok(())
} 