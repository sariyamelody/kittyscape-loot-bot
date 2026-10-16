mod command_handler;
mod prices;
mod collection_log;
mod config;
mod rank_manager;
mod logger;
mod runescape_tracker;
mod dink_listener;

use anyhow::Result;
use axum::Extension;
use axum::extract::DefaultBodyLimit;
use axum::routing::{get, post};
use axum::Router;
use serenity::all::{
    GatewayIntents, Interaction, Message, Ready, GuildId
};
use std::future::IntoFuture as _;
use serenity::async_trait;
use serenity::prelude::*;
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::SqlitePool;
use std::env;
use std::sync::Arc;
use dotenvy::dotenv;
use tracing::{error, info};
use command_handler::{PriceManagerKey, CollectionLogManagerKey};
use config::{Config, ConfigKey};
use runescape_tracker::RunescapeTrackerKey;

struct Handler {
    db: SqlitePool,
    price_manager: Arc<prices::PriceManager>,
    collection_log_manager: Arc<collection_log::CollectionLogManager>,
    runescape_tracker: Arc<runescape_tracker::RunescapeTracker>,
}

#[derive(Clone)]
struct DinkHandler {
    db: SqlitePool,
    price_manager: Arc<prices::PriceManager>,
    collection_log_manager: Arc<collection_log::CollectionLogManager>,
    runescape_tracker: Arc<runescape_tracker::RunescapeTracker>,
    ctx: Context,
    guild_id: GuildId,
    config: Config,
}

#[async_trait]
impl EventHandler for Handler {
    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        if let Err(why) = command_handler::handle_interaction(&ctx, &interaction, &self.db).await {
            error!("Error handling interaction: {:?}", why);
        }
    }

    async fn message(&self, ctx: Context, msg: Message) {
        // We only care about messages in the RuneLite plugin channel
        let data = ctx.data.read().await;
        if let Some(config) = data.get::<ConfigKey>() {
            if let Some(runelite_channel_id) = config.runelite_channel_id {
                if msg.channel_id == runelite_channel_id && msg.author.bot && msg.author.id != ctx.cache.current_user().id {
                    if let Err(why) = self.runescape_tracker.process_message(&ctx, &msg, &self.db).await {
                        error!("Error processing RuneLite message: {:?}", why);
                    }
                }
            }
        }
    }

    async fn ready(&self, ctx: Context, ready: Ready) {
        info!("{} is connected!", ready.user.name);

        // Store managers in context data
        {
            let mut data = ctx.data.write().await;
            data.insert::<PriceManagerKey>(Arc::clone(&self.price_manager));
            data.insert::<CollectionLogManagerKey>(Arc::clone(&self.collection_log_manager));
            data.insert::<RunescapeTrackerKey>(Arc::clone(&self.runescape_tracker));
        }

        // Register commands
        if let Err(why) = command_handler::register_commands(&ctx).await {
            error!("Error registering commands: {:?}", why);
        }

        let _ = crate::logger::log_generic(
            &ctx,
            "HELLKITTEN READY: Booted up and ready to serve, nya!"
        ).await;

        // Start price updates
        Arc::clone(&self.price_manager).start_price_updates().await;
    }
    async fn cache_ready(&self, ctx: Context, guilds: Vec<GuildId>) {
        info!("Cache built successfully!");

        // This bot only ever operates in one server (mod_channel_id, log_channel_id etc. are
        // all single-guild already), so rather than asking for a GUILD_ID env var that has to be
        // kept in sync by hand, just take the one guild serenity's cache already knows about.
        let Some(guild_id) = guilds.first().copied() else {
            error!("Bot is not in any guild - not starting the Dink listener");
            return;
        };
        if guilds.len() > 1 {
            error!("Bot is in {} guilds; picking {} for the Dink listener, which will misidentify members if that's the wrong one", guilds.len(), guild_id);
        }
        info!("Dink listener resolving members against guild {}", guild_id);

        let config = {
            let data = ctx.data.read().await;
            data.get::<ConfigKey>().expect("Config missing from client data").clone()
        };

        // tokio::spawn creates a new green thread that can run in parallel with the rest of
        // the application.
        let handler = DinkHandler {
            db: self.db.clone(),
            price_manager: Arc::clone(&self.price_manager),
            collection_log_manager: Arc::clone(&self.collection_log_manager),
            runescape_tracker: Arc::clone(&self.runescape_tracker),
            ctx,
            guild_id,
            config,
        };
        tokio::spawn(async move {
            // build our application with a single route, gated by a shared secret in the path -
            // Dink can't send custom headers, so the token has to live in the URL itself
            let app = Router::new().route("/dink/{token}", post(dink_listener::dink_handler))
                .route("/", get(|| async { "Hello, World!" }))
                .layer(DefaultBodyLimit::max(10 * 1024 * 1024))
                .layer(Extension(handler));
            // run our app with hyper, listening globally on port 3000
            let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
            let server = axum::serve(listener, app);
            if let Err(why) = server.into_future().await {
                error!("Err with server: {:?}", why);
            }
        });
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

    // Initialize config
    let config = Config::from_env()?;

    // Create database connection pool
    let db = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await?;

    // Run migrations
    sqlx::migrate!().run(&db).await?;

    // Initialize managers
    let price_manager = Arc::new(prices::PriceManager::new().await?);
    let collection_log_manager = Arc::new(collection_log::CollectionLogManager::new(&db).await?);
    let runescape_tracker = Arc::new(runescape_tracker::RunescapeTracker::new().await?);

    // Create a new instance of the client
    let intents = GatewayIntents::non_privileged() | GatewayIntents::MESSAGE_CONTENT | GatewayIntents::GUILD_MESSAGES;
    let mut client = Client::builder(&token, intents)
        .event_handler(Handler {
            db: db.clone(),
            price_manager: Arc::clone(&price_manager),
            collection_log_manager: Arc::clone(&collection_log_manager),
            runescape_tracker: Arc::clone(&runescape_tracker),
        })
        .await?;

    // Store config in client data
    {
        let mut data = client.data.write().await;
        data.insert::<ConfigKey>(config);
    }

    if let Err(why) = client.start().await {
        error!("Err with client: {:?}", why);
    }

    Ok(())
} 