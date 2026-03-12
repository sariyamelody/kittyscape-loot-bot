use anyhow::Result;use serenity::all::{
    AutocompleteChoice, Command, CommandOptionType, CreateAutocompleteResponse, CreateCommand, CreateCommandOption, CreateInteractionResponse, Interaction, Permissions
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
        .description("Check your points total"))
    .await?;

    Command::create_global_command(&ctx.http, CreateCommand::new("leaderboard")
        .description("View the points leaderboard"))
    .await?;

    Command::create_global_command(&ctx.http, CreateCommand::new("stats")
        .description("View detailed statistics for your account"))
    .await?;
        
    Command::create_global_command(&ctx.http, CreateCommand::new("rsname")
        .description("Link a RuneScape username to your Discord account")
        .add_option(CreateCommandOption::new(
            CommandOptionType::String,
            "username",
            "Your RuneScape username"
        )
        .required(true)))
    .await?;

    Command::create_global_command(&ctx.http, CreateCommand::new("rsname_remove")
        .description("Unlink a RuneScape username from your Discord account")
        .add_option(CreateCommandOption::new(
            CommandOptionType::String,
            "username",
            "The RuneScape username to unlink"
        )
        .required(true)))
    .await?;

    Command::create_global_command(&ctx.http, CreateCommand::new("rsnames")
        .description("List all RuneScape accounts linked to your Discord account"))
    .await?;

    let admin_permission_set = Permissions::ADMINISTRATOR;

    Command::create_global_command(&ctx.http, CreateCommand::new("recalculate")
        .description("ADMIN: Recalculate all points based on clamped categories.")
        .default_member_permissions(admin_permission_set))
    .await?;

    Command::create_global_command(&ctx.http, CreateCommand::new("clamp")
        .description("ADMIN: Clamp the points a category is allowed to give.")
        .default_member_permissions(admin_permission_set)
        .add_option(CreateCommandOption::new(
            CommandOptionType::String,
            "category",
            "The name of the category"
        )
        .required(true)
        .set_autocomplete(true)))
    .await?;

    Command::create_global_command(&ctx.http, CreateCommand::new("unclamp")
        .description("ADMIN: Unclamp the points a category is allowed to give.")
        .default_member_permissions(admin_permission_set)
        .add_option(CreateCommandOption::new(
            CommandOptionType::String,
            "category",
            "The name of the category"
        )
        .required(true)
        .set_autocomplete(true)))
    .await?;

    Command::create_global_command(&ctx.http, CreateCommand::new("whitelist")
        .description("ADMIN: Whitelist a clog item to never be clamped.")
        .default_member_permissions(admin_permission_set)
        .add_option(CreateCommandOption::new(
            CommandOptionType::String,
            "item",
            "The name of the collection log item"
        )
        .required(true)
        .set_autocomplete(true)))
    .await?;

    Command::create_global_command(&ctx.http, CreateCommand::new("unwhitelist")
        .description("ADMIN: Unwhitelist a clog item, so it will be clamped (if the category is clamped).")
        .default_member_permissions(admin_permission_set)
        .add_option(CreateCommandOption::new(
            CommandOptionType::String,
            "item",
            "The name of the collection log item"
        )
        .required(true)
        .set_autocomplete(true)))
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
                "rsname" => handle_rsname(command, ctx, db).await?,
                "rsname_remove" => handle_rsname_remove(command, ctx, db).await?,
                "rsnames" => handle_rsnames(command, ctx, db).await?,
                "recalculate" => handle_recalculate(command, ctx, db).await?,
                "clamp" => handle_clamp(command, ctx, db, true).await?,
                "unclamp" => handle_clamp(command, ctx, db, false).await?,
                "whitelist" => handle_whitelist(command, ctx, db, true).await?,
                "unwhitelist" => handle_whitelist(command, ctx, db, false).await?,
                _ => {
                    error!("Unknown command: {}", command.data.name);
                }
            }
        }
        Interaction::Autocomplete(autocomplete) => {
            match autocomplete.data.name.as_str() {
                "drop" | "clog" | "whitelist" | "unwhitelist" => {
                    if let Some(option) = autocomplete.data.options.iter().find(|opt| opt.name == "item" && opt.value.as_str().is_some()) {
                        if let Some(partial) = option.value.as_str() {
                            let data = ctx.data.read().await;
                            
                            let suggestions = if autocomplete.data.name == "drop" {
                                // Get price manager for drop suggestions
                                if let Some(price_manager) = data.get::<PriceManagerKey>() {
                                    price_manager.get_item_suggestions(partial).await
                                } else {
                                    Vec::new()
                                }
                            } else {
                                // Get collection log manager for clog suggestions
                                if let Some(clog_manager) = data.get::<CollectionLogManagerKey>() {
                                    clog_manager.get_suggestions(partial).await
                                } else {
                                    Vec::new()
                                }
                            };
                            
                            let choices: Vec<AutocompleteChoice> = suggestions
                                .into_iter()
                                .map(|item| AutocompleteChoice::new(item.clone(), item))
                                .collect();
                            
                            autocomplete.create_response(&ctx.http, 
                                CreateInteractionResponse::Autocomplete(
                                    CreateAutocompleteResponse::new().set_choices(choices)
                                )
                            ).await?;
                        }
                    }
                }
                "drop_remove" | "clog_remove" => {
                    if let Some(option) = autocomplete.data.options.iter().find(|opt| opt.name == "id") {
                        let discord_id = autocomplete.user.id.to_string();
                        
                        let recent_items = if autocomplete.data.name == "drop_remove" {
                            // Get recent drops
                            sqlx::query!(
                                "SELECT id, item_name, quantity, timestamp FROM drops 
                                 WHERE discord_id = ? 
                                 ORDER BY timestamp DESC 
                                 LIMIT 25",
                                discord_id
                            )
                            .fetch_all(db)
                            .await?
                            .into_iter()
                            .map(|row| {
                                let timestamp = row.timestamp.unwrap_or_default();
                                let id = row.id;
                                let name = row.item_name.clone();
                                let quantity = if row.quantity > 0 { row.quantity } else { 1i64 };
                                let display = if quantity > 1 {
                                    format!("#{}: {}x {} ({})", id, quantity, name, timestamp)
                                } else {
                                    format!("#{}: {} ({})", id, name, timestamp)
                                };
                                AutocompleteChoice::new(display, id)
                            })
                            .collect()
                        } else {
                            // Get recent clog entries
                            sqlx::query!(
                                "SELECT id, item_name, timestamp FROM collection_log_entries 
                                 WHERE discord_id = ? 
                                 ORDER BY timestamp DESC 
                                 LIMIT 25",
                                discord_id
                            )
                            .fetch_all(db)
                            .await?
                            .into_iter()
                            .map(|row| {
                                let timestamp = row.timestamp.unwrap_or_default();
                                let id = row.id;
                                let name = row.item_name.clone();
                                AutocompleteChoice::new(format!("#{}: {} ({})", id, name, timestamp), id)
                            })
                            .collect()
                        };
                        
                        autocomplete.create_response(&ctx.http, 
                            CreateInteractionResponse::Autocomplete(
                                CreateAutocompleteResponse::new().set_choices(recent_items)
                            )
                        ).await?;
                    }
                }
                "clamp" | "unclamp" => {
                    if let Some(option) = autocomplete.data.options.iter().find(|opt| opt.name == "category" && opt.value.as_str().is_some()) {
                        if let Some(partial) = option.value.as_str() {
                            let data = ctx.data.read().await;
                            
                            let suggestions = // Get collection log manager for clog suggestions
                                if let Some(clog_manager) = data.get::<CollectionLogManagerKey>() {
                                    clog_manager.get_category_suggestions(partial).await
                                } else {
                                    Vec::new()
                                }
                            ;
                            
                            let choices: Vec<AutocompleteChoice> = suggestions
                                .into_iter()
                                .map(|item| AutocompleteChoice::new(item.clone(), item))
                                .collect();
                            
                            autocomplete.create_response(&ctx.http, 
                                CreateInteractionResponse::Autocomplete(
                                    CreateAutocompleteResponse::new().set_choices(choices)
                                )
                            ).await?;
                        }
                    }
                }
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
