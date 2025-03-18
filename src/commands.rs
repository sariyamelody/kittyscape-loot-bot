use anyhow::Result;
use serenity::model::application::command::Command;
use serenity::model::application::interaction::{Interaction, InteractionResponseType};
use serenity::model::prelude::*;
use serenity::prelude::*;
use sqlx::SqlitePool;
use tracing::error;
use crate::prices::PriceManager;
use std::sync::Arc;

pub async fn register_commands(ctx: &Context) -> Result<()> {
    Command::create_global_application_command(&ctx.http, |command| {
        command
            .name("drop")
            .description("Record a new drop")
            .create_option(|option| {
                option
                    .name("item")
                    .description("The name of the item")
                    .kind(command::CommandOptionType::String)
                    .required(true)
                    .set_autocomplete(true)
            })
    })
    .await?;

    Command::create_global_application_command(&ctx.http, |command| {
        command
            .name("points")
            .description("Check your current points")
    })
    .await?;

    Command::create_global_application_command(&ctx.http, |command| {
        command
            .name("leaderboard")
            .description("View the top 10 looters")
    })
    .await?;

    Ok(())
}

pub async fn handle_interaction(ctx: &Context, interaction: &Interaction, db: &SqlitePool) -> Result<()> {
    match interaction {
        Interaction::ApplicationCommand(command) => {
            match command.data.name.as_str() {
                "drop" => {
                    let options = &command.data.options;
                    
                    let item_name = options
                        .iter()
                        .find(|opt| opt.name == "item")
                        .and_then(|opt| opt.value.as_ref()?.as_str())
                        .ok_or_else(|| anyhow::anyhow!("Item name not provided"))?;

                    // Get price manager from context data
                    let data = ctx.data.read().await;
                    let price_manager = data.get::<PriceManagerKey>()
                        .ok_or_else(|| anyhow::anyhow!("Price manager not found"))?;

                    // Look up the item price
                    let value = price_manager.get_item_price(item_name).await
                        .ok_or_else(|| anyhow::anyhow!("Item not found or no price data available"))?;

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

                    // Record the drop
                    sqlx::query!(
                        "INSERT INTO drops (discord_id, item_name, value) VALUES (?, ?, ?)",
                        discord_id,
                        item_name,
                        value
                    )
                    .execute(db)
                    .await?;

                    // Update user points
                    sqlx::query!(
                        "UPDATE users 
                         SET points = points + ?,
                             total_drops = total_drops + 1
                         WHERE discord_id = ?",
                        value,
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
                                    "Drop recorded: {} ({} gp)! You now have {} points. Next rank at {} points for {}!",
                                    item_name,
                                    value,
                                    user_points.points,
                                    rank.points,
                                    rank.role_name
                                )
                            }
                            None => {
                                format!(
                                    "Drop recorded: {} ({} gp)! You now have {} points!",
                                    item_name,
                                    value,
                                    user_points.points
                                )
                            }
                        }
                    } else {
                        format!(
                            "Drop recorded: {} ({} gp)! You now have {} points!",
                            item_name,
                            value,
                            user_points.points
                        )
                    };

                    command
                        .create_interaction_response(&ctx.http, |response| {
                            response
                                .kind(InteractionResponseType::ChannelMessageWithSource)
                                .interaction_response_data(|message| message.content(message_content))
                        })
                        .await?;
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
                        .create_interaction_response(&ctx.http, |response| {
                            response
                                .kind(InteractionResponseType::ChannelMessageWithSource)
                                .interaction_response_data(|message| message.content(message_content))
                        })
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
                        .create_interaction_response(&ctx.http, |response| {
                            response
                                .kind(InteractionResponseType::ChannelMessageWithSource)
                                .interaction_response_data(|message| message.content(message_content))
                        })
                        .await?;
                }
                _ => {
                    error!("Unknown command: {}", command.data.name);
                }
            }
        }
        Interaction::Autocomplete(autocomplete) => {
            if autocomplete.data.name == "drop" {
                if let Some(focused_option) = autocomplete.data.options.iter().find(|opt| opt.focused) {
                    if focused_option.name == "item" {
                        if let Some(partial) = focused_option.value.as_ref().and_then(|v| v.as_str()) {
                            let data = ctx.data.read().await;
                            let price_manager = data.get::<PriceManagerKey>()
                                .ok_or_else(|| anyhow::anyhow!("Price manager not found"))?;

                            let suggestions = price_manager.get_item_suggestions(partial).await;
                            
                            autocomplete
                                .create_autocomplete_response(&ctx.http, |response| {
                                    for name in suggestions {
                                        response.add_string_choice(&name, &name);
                                    }
                                    response
                                })
                                .await?;
                        }
                    }
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