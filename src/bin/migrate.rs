use anyhow::Result;
use dotenvy::dotenv;
use sqlx::sqlite::SqlitePoolOptions;
use std::env;
use tracing::info;
use tracing_subscriber;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize environment variables
    dotenv()?;

    // Initialize logging
    tracing_subscriber::fmt::init();

    info!("Running database migrations...");
    
    // Get database URL
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    
    // Create database connection pool
    let db = SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&database_url)
        .await?;

    // Run migrations
    sqlx::migrate!().run(&db).await?;

    info!("Migrations completed successfully!");
    
    Ok(())
} 