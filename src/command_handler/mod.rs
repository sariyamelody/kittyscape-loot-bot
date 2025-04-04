use anyhow::Result;
use serenity::all::{
    Command,
    CommandOptionType,
    Interaction,
    CreateCommand,
    CreateCommandOption,
    CreateInteractionResponse,
    CreateAutocompleteResponse,
    AutocompleteChoice,
};
use serenity::prelude::*;
use sqlx::SqlitePool;
use tracing::error;
use crate::prices::PriceManager;
use crate::collection_log::CollectionLogManager;
use std::sync::Arc;

mod commands;
mod utils;

pub use commands::*;
pub use utils::*;

pub async fn register_commands(ctx: &Context) -> Result<()> {
    Command::create_global_command(&ctx.http, CreateCommand::new("drop")
        .description("Record a new drop")
        .add_option(CreateCommandOption::new(
            CommandOptionType::String,
            "item",
            "The name of the item"
        )
        .required(true)
        .set_autocomplete(true))
        .add_option(CreateCommandOption::new(
            CommandOptionType::Integer,
            "quantity",
            "The quantity of items (default: 1)"
        )
        .required(false)
        .min_int_value(1)))
    .await?;

    Command::create_global_command(&ctx.http, CreateCommand::new("drop_remove")
        .description("Remove a mistakenly added drop")
        .add_option(CreateCommandOption::new(
            CommandOptionType::Integer,
            "id",
            "The ID of the drop to remove (leave empty to see recent drops)"
        )
        .required(false)
        .set_autocomplete(true)
        .min_int_value(1)))
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

    Command::create_global_command(&ctx.http, CreateCommand::new("clog_remove")
        .description("Remove a mistakenly added collection log entry")
        .add_option(CreateCommandOption::new(
            CommandOptionType::Integer,
            "id",
            "The ID of the collection log entry to remove (leave empty to see recent entries)"
        )
        .required(false)
        .set_autocomplete(true)
        .min_int_value(1)))
    .await?;

    Command::create_global_command(&ctx.http, CreateCommand::new("points")
        .description("Check your current points"))
    .await?;

    Command::create_global_command(&ctx.http, CreateCommand::new("leaderboard")
        .description("View the top 10 looters"))
    .await?;

    Command::create_global_command(&ctx.http, CreateCommand::new("stats")
        .description("View your detailed profile stats"))
    .await?;

    Ok(())
}

pub async fn handle_interaction(ctx: &Context, interaction: &Interaction, db: &SqlitePool) -> Result<()> {
    match interaction {
        Interaction::Command(command) => {
            match command.data.name.as_str() {
                "drop" => handle_drop(command, ctx, db).await?,
                "drop_remove" => handle_drop_remove(command, ctx, db).await?,
                "clog" => handle_clog(command, ctx, db).await?,
                "clog_remove" => handle_clog_remove(command, ctx, db).await?,
                "points" => handle_points(command, ctx, db).await?,
                "leaderboard" => handle_leaderboard(command, ctx, db).await?,
                "stats" => handle_stats(command, ctx, db).await?,
                _ => {
                    error!("Unknown command: {}", command.data.name);
                }
            }
        }
        Interaction::Autocomplete(autocomplete) => {
            match autocomplete.data.name.as_str() {
                // Handle item autocomplete for drop and clog commands
                "drop" | "clog" => {
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
                },
                // Handle ID autocomplete for drop_remove and clog_remove commands
                "drop_remove" => {
                    if let Some(_focused_option) = autocomplete.data.options.iter().find(|opt| opt.name == "id") {
                        let discord_id = autocomplete.user.id.to_string();
                        
                        // Get user's recent drops
                        let recent_drops = sqlx::query!(
                            "SELECT id, item_name, value, quantity 
                             FROM drops 
                             WHERE discord_id = ? 
                             ORDER BY timestamp DESC 
                             LIMIT 25",
                            discord_id
                        )
                        .fetch_all(db)
                        .await?;
                        
                        // Create suggestions in the format "ID: item_name (quantity x value)"
                        let suggestions: Vec<(String, i64)> = recent_drops.iter().map(|drop| {
                            let id = drop.id;
                            let label = format!(
                                "{}x {} ({})", 
                                drop.quantity, 
                                drop.item_name,
                                format_gp(drop.value)
                            );
                            (label, id)
                        }).collect();
                        
                        autocomplete
                            .create_response(&ctx.http, CreateInteractionResponse::Autocomplete(
                                CreateAutocompleteResponse::new()
                                    .set_choices(suggestions.into_iter().map(|(label, id)| {
                                        AutocompleteChoice::new(label, id)
                                    }).collect())
                            ))
                            .await?;
                    }
                },
                "clog_remove" => {
                    if let Some(_focused_option) = autocomplete.data.options.iter().find(|opt| opt.name == "id") {
                        let discord_id = autocomplete.user.id.to_string();
                        
                        // Get user's recent collection log entries
                        let recent_entries = sqlx::query!(
                            "SELECT id, item_name, points
                             FROM collection_log_entries 
                             WHERE discord_id = ? 
                             ORDER BY timestamp DESC 
                             LIMIT 25",
                            discord_id
                        )
                        .fetch_all(db)
                        .await?;
                        
                        // Create suggestions in the format "ID: item_name (points pts)"
                        let suggestions: Vec<(String, i64)> = recent_entries.iter().map(|entry| {
                            let id = entry.id;
                            let label = format!(
                                "{} ({} pts)", 
                                entry.item_name, 
                                entry.points
                            );
                            (label, id)
                        }).collect();
                        
                        autocomplete
                            .create_response(&ctx.http, CreateInteractionResponse::Autocomplete(
                                CreateAutocompleteResponse::new()
                                    .set_choices(suggestions.into_iter().map(|(label, id)| {
                                        AutocompleteChoice::new(label, id)
                                    }).collect())
                            ))
                            .await?;
                    }
                },
                _ => {}
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