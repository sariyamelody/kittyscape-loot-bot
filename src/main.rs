mod commands;
mod prices;

use anyhow::Result;
use dotenv::dotenv;
use serenity::async_trait;
use serenity::model::application::interaction::Interaction;
use serenity::model::gateway::Ready;
use serenity::prelude::*;
use sqlx::sqlite::SqlitePoolOptions;
use std::env;
use std::sync::Arc;
use tracing::{error, info};
use commands::PriceManagerKey;

struct Handler {
    db: sqlx::SqlitePool,
    price_manager: Arc<prices::PriceManager>,
}

#[async_trait]
impl EventHandler for Handler {
    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        if let Err(e) = commands::handle_interaction(&ctx, &interaction, &self.db).await {
            error!("Error handling interaction: {}", e);
        }
    }

    async fn ready(&self, ctx: Context, ready: Ready) {
        info!("{} is connected!", ready.user.name);

        // Store price manager in context data
        {
            let mut data = ctx.data.write().await;
            data.insert::<PriceManagerKey>(self.price_manager.clone());
        }

        if let Err(e) = commands::register_commands(&ctx).await {
            error!("Error registering commands: {}", e);
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenv()?;
    tracing_subscriber::fmt::init();

    info!("Starting bot...");

    let token = env::var("DISCORD_TOKEN")?;
    let db_url = env::var("DATABASE_URL")?;

    let db = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(&db_url)
        .await?;

    sqlx::migrate!().run(&db).await?;

    let price_manager = Arc::new(prices::PriceManager::new().await?);
    
    // Start the price update task
    Arc::clone(&price_manager).start_price_updates();

    let intents = GatewayIntents::GUILD_MESSAGES | GatewayIntents::MESSAGE_CONTENT;

    let mut client = Client::builder(&token, intents)
        .event_handler(Handler {
            db,
            price_manager,
        })
        .await?;

    client.start().await?;
    Ok(())
} 