mod command_handler;
mod prices;
mod collection_log;

use anyhow::Result;
use serenity::all::{
    GatewayIntents,
    Interaction,
    Ready,
};
use serenity::async_trait;
use serenity::prelude::*;
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::SqlitePool;
use std::env;
use std::sync::Arc;
use dotenvy::dotenv;
use tracing::{error, info};
use command_handler::{PriceManagerKey, CollectionLogManagerKey};

struct Handler {
    db: SqlitePool,
    price_manager: Arc<prices::PriceManager>,
    collection_log_manager: Arc<collection_log::CollectionLogManager>,
}

#[async_trait]
impl EventHandler for Handler {
    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        if let Err(why) = command_handler::handle_interaction(&ctx, &interaction, &self.db).await {
            error!("Error handling interaction: {:?}", why);
        }
    }

    async fn ready(&self, ctx: Context, ready: Ready) {
        info!("{} is connected!", ready.user.name);

        // Store managers in context data
        {
            let mut data = ctx.data.write().await;
            data.insert::<PriceManagerKey>(Arc::clone(&self.price_manager));
            data.insert::<CollectionLogManagerKey>(Arc::clone(&self.collection_log_manager));
        }

        // Register commands
        if let Err(why) = command_handler::register_commands(&ctx).await {
            error!("Error registering commands: {:?}", why);
        }

        // Start price updates
        Arc::clone(&self.price_manager).start_price_updates().await;
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize environment variables
    dotenv()?;

    // Initialize logging
    tracing_subscriber::fmt::init();

    info!("Starting bot...");

    // Get the token from the environment variable
    let token = env::var("DISCORD_TOKEN")?;
    let database_url = env::var("DATABASE_URL")?;

    // Create database connection pool
    let db = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await?;

    // Run migrations
    sqlx::migrate!().run(&db).await?;

    // Initialize managers
    let price_manager = Arc::new(prices::PriceManager::new().await?);
    let collection_log_manager = Arc::new(collection_log::CollectionLogManager::new().await?);

    // Create a new instance of the client
    let intents = GatewayIntents::non_privileged();
    let mut client = Client::builder(&token, intents)
        .event_handler(Handler {
            db: db.clone(),
            price_manager: Arc::clone(&price_manager),
            collection_log_manager: Arc::clone(&collection_log_manager),
        })
        .await?;

    // Start the client
    if let Err(why) = client.start().await {
        error!("Client error: {:?}", why);
    }

    Ok(())
} 