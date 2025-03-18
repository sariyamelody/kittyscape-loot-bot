use anyhow::Result;
use serenity::all::{
    Command,
    CommandOptionType,
    Interaction,
    CreateCommand,
    CreateCommandOption,
    CreateInteractionResponse,
    CreateInteractionResponseMessage,
    CreateAutocompleteResponse,
    AutocompleteChoice,
};
use serenity::prelude::*;
use sqlx::SqlitePool;
use tracing::error;
use crate::prices::PriceManager;
use crate::collection_log::CollectionLogManager;
use std::sync::Arc;

pub async fn register_commands(ctx: &Context) -> Result<()> {
    Command::create_global_command(&ctx.http, CreateCommand::new("drop")
        .description("Record a new drop")
        .add_option(CreateCommandOption::new(
            CommandOptionType::String,
            "item",
            "The name of the item"
        )
        .required(true)
        .set_autocomplete(true)))
    .await?;

    Command::create_global_command(&ctx.http, CreateCommand::new("clog")
        .description("Record a collection log item")
        .add_option(CreateCommandOption::new(
            CommandOptionType::String,
            "item",
            "The name of the collection log item"
        )
        .required(true)
        .set_autocomplete(true)))
    .await?;

    Command::create_global_command(&ctx.http, CreateCommand::new("points")
        .description("Check your current points"))
    .await?;

    Command::create_global_command(&ctx.http, CreateCommand::new("leaderboard")
        .description("View the top 10 looters"))
    .await?;

    Ok(())
}

pub async fn handle_interaction(ctx: &Context, interaction: &Interaction, db: &SqlitePool) -> Result<()> {
    match interaction {
        Interaction::Command(command) => {
            match command.data.name.as_str() {
                "drop" => {
                    let options = &command.data.options;
                    
                    let item_name = options
                        .iter()
                        .find(|opt| opt.name == "item")
                        .and_then(|opt| opt.value.as_str())
                        .ok_or_else(|| anyhow::anyhow!("Item name not provided"))?;

                    // Get collection log manager from context data
                    let data = ctx.data.read().await;
                    let collection_log_manager = data.get::<CollectionLogManagerKey>()
                        .ok_or_else(|| anyhow::anyhow!("Collection log manager not found"))?;

                    // Calculate collection log points
                    if let Some(points) = collection_log_manager.calculate_points(item_name).await {
                        let discord_id = command.user.id.to_string();

                        // Insert or update user
                        sqlx::query!(
                            "INSERT INTO users (discord_id, points, total_drops) 
                             VALUES (?, 0, 0)
                             ON CONFLICT(discord_id) DO NOTHING",
                            discord_id
                        )
                        .execute(db)
                        .await?;

                        // Record the collection log entry
                        sqlx::query!(
                            "INSERT INTO collection_log_entries (discord_id, item_name, points) VALUES (?, ?, ?)",
                            discord_id,
                            item_name,
                            points
                        )
                        .execute(db)
                        .await?;

                        // Update user points
                        sqlx::query!(
                            "UPDATE users 
                             SET points = points + ?
                             WHERE discord_id = ?",
                            points,
                            discord_id
                        )
                        .execute(db)
                        .await?;

                        // Check for rank up
                        let user_points = sqlx::query!(
                            "SELECT points FROM users WHERE discord_id = ?",
                            discord_id
                        )
                        .fetch_one(db)
                        .await?;

                        // Get the next rank threshold
                        let message_content = if let Ok(next_rank) = sqlx::query!(
                            "SELECT points, role_name FROM rank_thresholds 
                             WHERE points > ? 
                             ORDER BY points ASC 
                             LIMIT 1",
                            user_points.points
                        )
                        .fetch_optional(db)
                        .await
                        {
                            match next_rank {
                                Some(rank) => {
                                    format!(
                                        "Collection log entry recorded: {} (+{} points)! You now have {} points. Next rank at {} points for {}!",
                                        item_name,
                                        points,
                                        user_points.points,
                                        rank.points,
                                        rank.role_name
                                    )
                                }
                                None => {
                                    format!(
                                        "Collection log entry recorded: {} (+{} points)! You now have {} points!",
                                        item_name,
                                        points,
                                        user_points.points
                                    )
                                }
                            }
                        } else {
                            format!(
                                "Collection log entry recorded: {} (+{} points)! You now have {} points!",
                                item_name,
                                points,
                                user_points.points
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
                                    .content(format!("Item '{}' not found in collection log.", item_name))
                            ))
                            .await?;
                    }
                }
                "clog" => {
                    let options = &command.data.options;
                    
                    let item_name = options
                        .iter()
                        .find(|opt| opt.name == "item")
                        .and_then(|opt| opt.value.as_str())
                        .ok_or_else(|| anyhow::anyhow!("Item name not provided"))?;

                    // Get collection log manager from context data
                    let data = ctx.data.read().await;
                    let collection_log_manager = data.get::<CollectionLogManagerKey>()
                        .ok_or_else(|| anyhow::anyhow!("Collection log manager not found"))?;

                    // Calculate collection log points
                    if let Some(points) = collection_log_manager.calculate_points(item_name).await {
                        let discord_id = command.user.id.to_string();

                        // Insert or update user
                        sqlx::query!(
                            "INSERT INTO users (discord_id, points, total_drops) 
                             VALUES (?, 0, 0)
                             ON CONFLICT(discord_id) DO NOTHING",
                            discord_id
                        )
                        .execute(db)
                        .await?;

                        // Record the collection log entry
                        sqlx::query!(
                            "INSERT INTO collection_log_entries (discord_id, item_name, points) VALUES (?, ?, ?)",
                            discord_id,
                            item_name,
                            points
                        )
                        .execute(db)
                        .await?;

                        // Update user points
                        sqlx::query!(
                            "UPDATE users 
                             SET points = points + ?
                             WHERE discord_id = ?",
                            points,
                            discord_id
                        )
                        .execute(db)
                        .await?;

                        // Check for rank up
                        let user_points = sqlx::query!(
                            "SELECT points FROM users WHERE discord_id = ?",
                            discord_id
                        )
                        .fetch_one(db)
                        .await?;

                        // Get the next rank threshold
                        let message_content = if let Ok(next_rank) = sqlx::query!(
                            "SELECT points, role_name FROM rank_thresholds 
                             WHERE points > ? 
                             ORDER BY points ASC 
                             LIMIT 1",
                            user_points.points
                        )
                        .fetch_optional(db)
                        .await
                        {
                            match next_rank {
                                Some(rank) => {
                                    format!(
                                        "Collection log entry recorded: {} (+{} points)! You now have {} points. Next rank at {} points for {}!",
                                        item_name,
                                        points,
                                        user_points.points,
                                        rank.points,
                                        rank.role_name
                                    )
                                }
                                None => {
                                    format!(
                                        "Collection log entry recorded: {} (+{} points)! You now have {} points!",
                                        item_name,
                                        points,
                                        user_points.points
                                    )
                                }
                            }
                        } else {
                            format!(
                                "Collection log entry recorded: {} (+{} points)! You now have {} points!",
                                item_name,
                                points,
                                user_points.points
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
                                    .content(format!("Item '{}' not found in collection log.", item_name))
                            ))
                            .await?;
                    }
                }
                "points" => {
                    let discord_id = command.user.id.to_string();
                    let user_data = sqlx::query!(
                        "SELECT points, total_drops FROM users WHERE discord_id = ?",
                        discord_id
                    )
                    .fetch_optional(db)
                    .await?;

                    let message_content = match user_data {
                        Some(data) => {
                            format!(
                                "You have {} points from {} total drops!",
                                data.points, data.total_drops
                            )
                        }
                        None => "You haven't recorded any drops yet!".to_string(),
                    };

                    command
                        .create_response(&ctx.http, CreateInteractionResponse::Message(
                            CreateInteractionResponseMessage::new().content(message_content)
                        ))
                        .await?;
                }
                "leaderboard" => {
                    let top_users = sqlx::query!(
                        "SELECT discord_id, points, total_drops 
                         FROM users 
                         ORDER BY points DESC 
                         LIMIT 10"
                    )
                    .fetch_all(db)
                    .await?;

                    let mut message_content = String::from("🏆 **Top 10 Looters** 🏆\n");
                    for (i, user) in top_users.iter().enumerate() {
                        message_content.push_str(&format!(
                            "{}. <@{}> - {} points ({} drops)\n",
                            i + 1,
                            user.discord_id.as_ref().unwrap_or(&"Unknown".to_string()),
                            user.points,
                            user.total_drops
                        ));
                    }

                    command
                        .create_response(&ctx.http, CreateInteractionResponse::Message(
                            CreateInteractionResponseMessage::new().content(message_content)
                        ))
                        .await?;
                }
                _ => {
                    error!("Unknown command: {}", command.data.name);
                }
            }
        }
        Interaction::Autocomplete(autocomplete) => {
            if let Some(focused_option) = autocomplete.data.options.iter().find(|opt| opt.name == "item") {
                if let Some(partial) = focused_option.value.as_str() {
                    let data = ctx.data.read().await;
                    
                    let suggestions = if autocomplete.data.name == "clog" {
                        // Get collection log suggestions
                        let collection_log_manager = data.get::<CollectionLogManagerKey>()
                            .ok_or_else(|| anyhow::anyhow!("Collection log manager not found"))?;
                        collection_log_manager.get_suggestions(partial).await
                    } else {
                        // Get regular item suggestions for drops
                        let price_manager = data.get::<PriceManagerKey>()
                            .ok_or_else(|| anyhow::anyhow!("Price manager not found"))?;
                        price_manager.get_item_suggestions(partial).await
                    };
                    
                    autocomplete
                        .create_response(&ctx.http, CreateInteractionResponse::Autocomplete(
                            CreateAutocompleteResponse::new()
                                .set_choices(suggestions.into_iter().map(|name| {
                                    AutocompleteChoice::new(name.clone(), name)
                                }).collect())
                        ))
                        .await?;
                }
            }
        }
        _ => {}
    }

    Ok(())
}

pub struct PriceManagerKey;

impl TypeMapKey for PriceManagerKey {
    type Value = Arc<PriceManager>;
}

pub struct CollectionLogManagerKey;

impl TypeMapKey for CollectionLogManagerKey {
    type Value = Arc<CollectionLogManager>;
} 