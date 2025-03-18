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
                "clog" => handle_clog(command, ctx, db).await?,
                "points" => handle_points(command, ctx, db).await?,
                "leaderboard" => handle_leaderboard(command, ctx, db).await?,
                "stats" => handle_stats(command, ctx, db).await?,
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