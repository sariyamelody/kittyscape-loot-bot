use anyhow::Result;
use regex::Regex;
use serenity::all::Message;
use serenity::prelude::*;
use sqlx::SqlitePool;
use std::sync::Arc;
use lazy_static::lazy_static;
use tracing::{info, warn, error, debug};
use crate::rank_manager;

lazy_static! {
    // Regular expressions for parsing messages from RuneLite plugins
    static ref DROP_REGEX: Regex = Regex::new(r"^(.+) received: (.+)(?: \((\d+)x\))? \(([0-9,]+) coins\)$").unwrap();
    static ref CLOG_REGEX: Regex = Regex::new(r"(?:\*\*(.+)\*\*\s+New item added to your collection log: \*\*(.+)\*\*|^(.+) received a collection log item: (.+)$)").unwrap();
    
    // Regex for parsing embed drop notifications
    // This pattern handles formats like:
    // "Just got [Coal] from [Monster]"
    // "Just got 5x [Coal] from [Monster]"
    // "Just got [Coal](url) from [Monster](url)"
    // "Just got 5x [Coal](url) from lvl 98 [Monster](url)"
    static ref EMBED_DROP_REGEX: Regex = Regex::new(r"Just got (?:(\d+)x\s+)?\[(.+?)(?:\]\(.+?\)|\])\s+from(?:\s+lvl\s+\d+)?\s+\[(.+?)(?:\]\(.+?\)|\])").unwrap();
    static ref EMBED_VALUE_REGEX: Regex = Regex::new(r"```fix\s*([0-9,]+) GP\s*```").unwrap();
}

pub struct RunescapeTracker {
    // Add fields as needed
}

impl RunescapeTracker {
    pub async fn new() -> Result<Self> {
        Ok(Self {})
    }

    pub async fn process_message(&self, ctx: &Context, msg: &Message, db: &SqlitePool) -> Result<()> {
        let content = &msg.content;
        debug!("Processing message in RuneLite channel: ID={}, Author={}, Content={}", msg.id, msg.author.name, content);
        
        // Check if the message has embeds
        if !msg.embeds.is_empty() {
            debug!("Message has {} embeds", msg.embeds.len());
            
            // Process each embed
            for (i, embed) in msg.embeds.iter().enumerate() {
                debug!("Processing embed #{}", i + 1);
                
                // Check for embed description
                if let Some(description) = &embed.description {
                    debug!("Embed description: {}", description);
                    
                    // Try to extract item name and source from the description
                    if let Some(captures) = EMBED_DROP_REGEX.captures(description) {
                        let item_name = captures.get(2).map_or("", |m| m.as_str()).trim();
                        let source = captures.get(3).map_or("", |m| m.as_str()).trim();
                        debug!("Found item in embed: {}", item_name);
                        debug!("From source: {}", source);
                        
                        // Parse quantity - default to 1 if not present
                        let quantity: i64 = captures.get(1).map_or("1", |m| m.as_str()).parse().unwrap_or(1);
                        debug!("Quantity: {}", quantity);
                        
                        // Try to extract the value from the GE Value field
                        let mut value: i64 = 0;
                        for field in &embed.fields {
                            if field.name == "GE Value" && !field.value.is_empty() {
                                debug!("Found GE Value field: {}", field.value);
                                if let Some(value_capture) = EMBED_VALUE_REGEX.captures(&field.value) {
                                    let value_str = value_capture.get(1).map_or("0", |m| m.as_str()).replace(",", "");
                                    value = value_str.parse().unwrap_or(0);
                                    debug!("Parsed value: {}", value);
                                }
                            }
                        }
                        
                        // Try to get the RS name from the embed author if it exists
                        let mut rs_name = String::new();
                        if let Some(author) = &embed.author {
                            debug!("Found embed author: {}", author.name);
                            if !author.name.is_empty() {
                                rs_name = author.name.clone();
                                debug!("Using RS name from embed author: {}", rs_name);
                            }
                        } else {
                            debug!("No author field found in embed");
                        }
                        
                        // If no RS name found in author, we can't process this embed
                        // since the message author will just be the webhook bot
                        if rs_name.is_empty() {
                            debug!("No RS name found in embed author for item: {}", item_name);
                            return Ok(());
                        }
                        
                        debug!("Processing drop from embed: {} received {}x {} worth {} from {}", 
                               rs_name, quantity, item_name, value, source);
                        // Process the drop notification from the embed
                        self.process_drop(ctx, &rs_name, item_name, quantity, value, db, msg).await?;
                        return Ok(());
                    } else {
                        debug!("Could not find drop pattern in embed description");
                    }
                } else {
                    debug!("Embed has no description");
                }
            }
        }
        
        // Try to parse drop message from plain text
        if let Some(captures) = DROP_REGEX.captures(content) {
            let rs_name = captures.get(1).map_or("", |m| m.as_str()).trim();
            let item_name = captures.get(2).map_or("", |m| m.as_str()).trim();
            let quantity: i64 = captures.get(3)
                .map_or("1", |m| m.as_str())
                .parse()
                .unwrap_or(1);
            let value_str = captures.get(4).map_or("0", |m| m.as_str()).replace(",", "");
            let value: i64 = value_str.parse().unwrap_or(0);
            
            debug!("Parsed drop from text: {} received {}x {} worth {}", rs_name, quantity, item_name, value);
            // Process the drop notification
            self.process_drop(ctx, rs_name, item_name, quantity, value, db, msg).await?;
            return Ok(());
        }
        
        // Try to parse collection log message
        if let Some(captures) = CLOG_REGEX.captures(content) {
            // Try the new format first (with bold/asterisks)
            let (rs_name, item_name) = if let (Some(name), Some(item)) = (captures.get(1), captures.get(2)) {
                (name.as_str().trim(), item.as_str().trim())
            } 
            // Fall back to the original format
            else if let (Some(name), Some(item)) = (captures.get(3), captures.get(4)) {
                (name.as_str().trim(), item.as_str().trim())
            } else {
                ("", "")
            };
            
            if !rs_name.is_empty() && !item_name.is_empty() {
                debug!("Parsed collection log from text: {} received {}", rs_name, item_name);
                // Process the collection log notification
                self.process_clog(ctx, rs_name, item_name, db, msg).await?;
                return Ok(());
            }
        }
        
        // If we get here, we couldn't parse the message
        debug!("Could not parse message format: {}", content);
        Ok(())
    }
    
    async fn process_drop(
        &self,
        ctx: &Context,
        rs_name: &str,
        item_name: &str,
        quantity: i64,
        value: i64,
        db: &SqlitePool,
        original_msg: &Message
    ) -> Result<()> {
        debug!("Processing drop for {} - Item: {}, Quantity: {}, Value: {}", rs_name, item_name, quantity, value);

        // Look up the Discord ID for this Runescape username
        let discord_ids = self.get_discord_ids_for_rs_name(rs_name, db).await?;
        
        if discord_ids.is_empty() {
            debug!("No Discord account linked to RS name '{}' for drop: {}", rs_name, item_name);
            return Ok(());
        }

        debug!("Found {} Discord accounts linked to RS name '{}'", discord_ids.len(), rs_name);
        
        // Process drop for each linked Discord account
        for discord_id in discord_ids {
            debug!("Processing drop for Discord ID: {}", discord_id);
            
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
                "INSERT INTO drops (discord_id, item_name, value, quantity) VALUES (?, ?, ?, ?)",
                discord_id,
                item_name,
                value,
                quantity
            )
            .execute(db)
            .await?;

            // Update total drops
            sqlx::query!(
                "UPDATE users 
                 SET total_drops = total_drops + ?
                 WHERE discord_id = ?",
                quantity,
                discord_id
            )
            .execute(db)
            .await?;

            // Calculate points (1 point per 100K gp)
            let points = value / 100_000;
            
            // Get user name for rank updates
            let user_name = match self.get_username_from_discord_id(ctx, &discord_id).await {
                Ok(name) => name,
                Err(_) => format!("Unknown ({})", discord_id),
            };

            // Add points and check for rank up
            debug!("Adding {} points to {} ({})", points, user_name, discord_id);
            rank_manager::add_points(
                ctx,
                &discord_id,
                &user_name,
                points,
                db
            ).await?;
            
            debug!("Auto-added drop for {}: {}x {} worth {} GP (Discord ID: {})", 
                  rs_name, quantity, item_name, value, discord_id);
                  
            // Log the auto-added drop to the bot log channel
            crate::logger::log_action(
                ctx,
                &discord_id,
                "AUTO-DROP",
                &format!("{} received {}x {} worth {} GP", rs_name, quantity, item_name, value)
            ).await?;
        }
        
        // Add a checkmark reaction to the original message
        let _ = original_msg.react(ctx, '✅').await;
        
        Ok(())
    }
    
    async fn process_clog(
        &self,
        ctx: &Context,
        rs_name: &str,
        item_name: &str,
        db: &SqlitePool,
        original_msg: &Message
    ) -> Result<()> {
        debug!("Processing collection log entry for {} - Item: {}", rs_name, item_name);
        // Look up the Discord ID for this Runescape username
        let discord_ids = self.get_discord_ids_for_rs_name(rs_name, db).await?;
        
        if discord_ids.is_empty() {
            debug!("No Discord account linked to RS name '{}' for clog: {}", rs_name, item_name);
            return Ok(());
        }
        
        debug!("Found {} Discord accounts linked to RS name '{}' for collection log", discord_ids.len(), rs_name);
        
        // Get collection log manager from context data
        let data = ctx.data.read().await;
        let collection_log_manager = match data.get::<crate::command_handler::CollectionLogManagerKey>() {
            Some(manager) => manager,
            None => {
                error!("Collection log manager not found");
                return Ok(());
            }
        };
        
        // Calculate collection log points
        let points = match collection_log_manager.calculate_points(item_name).await {
            Some(pts) => {
                debug!("Calculated {} points for collection log item: {}", pts, item_name);
                pts
            },
            None => {
                warn!("Could not calculate points for clog item: {}", item_name);
                return Ok(());
            }
        };
        
        // Process clog for each linked Discord account
        for discord_id in discord_ids {
            debug!("Processing collection log for Discord ID: {}", discord_id);
            
            // Check if user already has this collection log entry
            if let Ok(Some(_)) = sqlx::query!(
                "SELECT id FROM collection_log_entries 
                WHERE discord_id = ? AND item_name = ?",
                discord_id,
                item_name
            )
            .fetch_optional(db)
            .await
            {
                debug!("User {} already has collection log entry for {}", discord_id, item_name);
                continue;
            }
            
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
            
            // Get user name for rank updates
            let user_name = match self.get_username_from_discord_id(ctx, &discord_id).await {
                Ok(name) => name,
                Err(_) => format!("Unknown ({})", discord_id),
            };

            // Add points and check for rank up
            debug!("Adding {} points to {} ({}) for collection log item", points, user_name, discord_id);
            rank_manager::add_points(
                ctx,
                &discord_id,
                &user_name,
                points,
                db
            ).await?;
            
            debug!("Auto-added collection log entry for {}: {} (+{} points) (Discord ID: {})", 
                  rs_name, item_name, points, discord_id);
                  
            // Log the auto-added collection log entry to the bot log channel
            crate::logger::log_action(
                ctx,
                &discord_id,
                "AUTO-CLOG",
                &format!("{} received collection log item: {} (+{} points)", rs_name, item_name, points)
            ).await?;
        }
        
        // Add a checkmark reaction to the original message
        let _ = original_msg.react(ctx, '✅').await;
        
        Ok(())
    }
    
    async fn get_discord_ids_for_rs_name(&self, rs_name: &str, db: &SqlitePool) -> Result<Vec<String>> {
        debug!("Looking up Discord IDs for RS name: {}", rs_name);
        
        let records = sqlx::query!(
            "SELECT discord_id FROM runescape_accounts WHERE runescape_name = ? COLLATE NOCASE",
            rs_name
        )
        .fetch_all(db)
        .await?;
        
        let discord_ids: Vec<String> = records.into_iter()
            .map(|record| record.discord_id)
            .collect();
            
        debug!("Found {} Discord IDs linked to RS name '{}'", discord_ids.len(), rs_name);
        
        for (i, id) in discord_ids.iter().enumerate() {
            debug!(" - Linked Discord ID #{}: {}", i + 1, id);
        }
        
        Ok(discord_ids)
    }
    
    pub async fn get_username_from_discord_id(&self, ctx: &Context, discord_id: &str) -> Result<String> {
        let user_id = discord_id.parse::<u64>()
            .map_err(|_| anyhow::anyhow!("Invalid Discord ID"))?;
        
        let user = ctx.http.get_user(serenity::all::UserId::new(user_id)).await?;
        
        Ok(user.name)
    }


    async fn get_rs_name_for_author(&self, author_id: &str, db: &SqlitePool) -> Result<String> {
        let records = sqlx::query!(
            "SELECT runescape_name FROM runescape_accounts WHERE discord_id = ? LIMIT 1",
            author_id
        )
        .fetch_all(db)
        .await?;
        
        Ok(records.first()
            .and_then(|record| Some(record.runescape_name.clone()))
            .unwrap_or_default())
    }
}

pub struct RunescapeTrackerKey;

impl TypeMapKey for RunescapeTrackerKey {
    type Value = Arc<RunescapeTracker>;
} 