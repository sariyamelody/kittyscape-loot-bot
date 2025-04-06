use anyhow::Result;
use dotenvy::dotenv;
use lazy_static::lazy_static;
use regex::Regex;
use serenity::all::{ChannelId, GatewayIntents, GetMessages, Http};
use serenity::prelude::*;
use std::env;
use tracing::{info, warn, debug};
use tracing_subscriber;

lazy_static! {
    // Original regex patterns
    static ref DROP_REGEX: Regex = Regex::new(r"^(.+) received: (.+)(?: \((\d+)x\))? \(([0-9,]+) coins\)$").unwrap();
    static ref CLOG_REGEX: Regex = Regex::new(r"^(.+) received a collection log item: (.+)$").unwrap();
    
    // Additional regex patterns to test
    static ref DROP_REGEX_ALT1: Regex = Regex::new(r"(.+) received drop: (.+)(?: \((\d+)x\))? \(([0-9,]+) coins\)").unwrap();
    static ref DROP_REGEX_ALT2: Regex = Regex::new(r"(.+) received .* drop: (.+)(?: \((\d+)x\))? \(([0-9,]+) coins\)").unwrap();
    static ref DROP_REGEX_ALT3: Regex = Regex::new(r"(.+) received .* drop: (.+)(?: \((\d+)x\))? \(([0-9,]+)gp\)").unwrap();
    static ref CLOG_REGEX_ALT1: Regex = Regex::new(r"(.+) received collection log item: (.+)").unwrap();
    static ref CLOG_REGEX_ALT2: Regex = Regex::new(r"(.+) received a new collection log item: (.+)").unwrap();
    static ref CLOG_REGEX_ALT3: Regex = Regex::new(r"\*\*(.+)\*\*\s+New item added to your collection log: \*\*(.+)\*\*").unwrap();
    
    // Embed regex patterns
    static ref EMBED_DROP_REGEX: Regex = Regex::new(r"Just got \[(.+?)\].+? from \[(.+?)\]").unwrap();
    static ref EMBED_VALUE_REGEX: Regex = Regex::new(r"```fix\s*([0-9,]+) GP\s*```").unwrap();
}

// RuneLite webhook/bot ID
const RUNELITE_BOT_ID: u64 = 1351642107730722866;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize environment variables
    dotenv()?;

    // Initialize logging
    tracing_subscriber::fmt::init();

    // Get Discord token
    let token = env::var("DISCORD_TOKEN").expect("Expected DISCORD_TOKEN in environment");
    
    // Get RuneLite channel ID
    let channel_id_str = env::var("RUNELITE_CHANNEL_ID")
        .expect("Expected RUNELITE_CHANNEL_ID in environment");
    let channel_id = ChannelId::new(
        channel_id_str.parse::<u64>()
            .expect("RUNELITE_CHANNEL_ID must be a valid u64")
    );
    
    // Get number of messages to analyze (default to 100)
    let limit = env::var("ANALYZE_LIMIT")
        .ok()
        .and_then(|s| s.parse::<u8>().ok())
        .unwrap_or(100);

    // Create minimal client (just for API access, not for event handling)
    info!("Creating Discord client...");
    let intents = GatewayIntents::empty();
    let client = Client::builder(&token, intents)
        .await?;
    
    info!("Starting analysis for channel {}...", channel_id);
    analyze_channel_history(&client.http, channel_id, limit).await?;

    Ok(())
}

async fn analyze_channel_history(http: &Http, channel_id: ChannelId, limit: u8) -> Result<()> {
    info!("Fetching last {} messages from channel {}", limit, channel_id);
    
    // Get channel messages
    let messages = channel_id.messages(http, GetMessages::new().limit(limit)).await?;
    
    info!("Analyzing {} messages", messages.len());
    
    // Message counters
    let mut runelite_messages = 0;
    let mut user_messages = 0;
    let mut other_bot_messages = 0;
    
    // Counters for original regexes
    let mut drop_matches = 0;
    let mut clog_matches = 0;
    
    // Counters for alternative regexes
    let mut drop_alt1_matches = 0;
    let mut drop_alt2_matches = 0;
    let mut drop_alt3_matches = 0;
    let mut clog_alt1_matches = 0;
    let mut clog_alt2_matches = 0;
    let mut clog_alt3_matches = 0;
    
    let mut unknown_formats = Vec::new();
    
    for msg in messages {
        let author_id = msg.author.id.get();
        let is_bot = msg.author.bot;
        let content = &msg.content;
        
        // Categorize message by author
        if author_id == RUNELITE_BOT_ID {
            runelite_messages += 1;
            info!("RuneLite message: Author: {}, Content: {}", msg.author.name, content);
            
            // Variable to track if any part of this message matched our patterns
            let mut message_matched = false;
            
            // Check if the message has embeds
            if !msg.embeds.is_empty() {
                let embed_count = msg.embeds.len();
                info!("Message has {} embed(s)", embed_count);
                
                // Process each embed
                for (i, embed) in msg.embeds.iter().enumerate() {
                    info!("Embed #{} details:", i + 1);
                    
                    // Check for author information
                    if let Some(author) = &embed.author {
                        info!("  Author Name: {}", author.name);
                        if let Some(author_url) = &author.url {
                            info!("  Author URL: {}", author_url);
                        }
                        if let Some(author_icon_url) = &author.icon_url {
                            info!("  Author Icon URL: {}", author_icon_url);
                        }
                    }
                    
                    if let Some(title) = &embed.title {
                        info!("  Title: {}", title);
                    }
                    
                    if let Some(description) = &embed.description {
                        info!("  Description: {}", description);
                        
                        // Test for embed drop regex patterns
                        if EMBED_DROP_REGEX.is_match(description) {
                            info!("EMBED DROP: {}", description);
                            
                            if let Some(captures) = EMBED_DROP_REGEX.captures(description) {
                                let item = captures.get(1).map_or("", |m| m.as_str()).trim();
                                let source = captures.get(2).map_or("", |m| m.as_str()).trim();
                                
                                info!("  Item: {}", item);
                                info!("  Source: {}", source);
                                
                                // Look for GE Value in fields
                                for field in &embed.fields {
                                    if field.name == "GE Value" && EMBED_VALUE_REGEX.is_match(&field.value) {
                                        if let Some(value_capture) = EMBED_VALUE_REGEX.captures(&field.value) {
                                            let value = value_capture.get(1).map_or("0", |m| m.as_str());
                                            info!("  Value: {} GP", value);
                                        }
                                    }
                                }
                                message_matched = true;
                            }
                        }
                        
                        // Apply our regex patterns to the description
                        let mut embed_matched = false;
                        process_content(description, &mut drop_matches, &mut clog_matches, 
                            &mut drop_alt1_matches, &mut drop_alt2_matches, &mut drop_alt3_matches,
                            &mut clog_alt1_matches, &mut clog_alt2_matches, &mut clog_alt3_matches,
                            &mut unknown_formats, &mut embed_matched);
                        message_matched |= embed_matched;
                    }
                    
                    // Check embed fields
                    if !embed.fields.is_empty() {
                        info!("  Fields count: {}", embed.fields.len());
                        for field in &embed.fields {
                            info!("    Field name: {}", field.name);
                            info!("    Field value: {}", field.value);
                            
                            // Apply our regex patterns to the field name and value
                            let mut name_matched = false;
                            process_content(&field.name, &mut drop_matches, &mut clog_matches, 
                                &mut drop_alt1_matches, &mut drop_alt2_matches, &mut drop_alt3_matches,
                                &mut clog_alt1_matches, &mut clog_alt2_matches, &mut clog_alt3_matches,
                                &mut unknown_formats, &mut name_matched);
                            message_matched |= name_matched;
                                
                            let mut value_matched = false;
                            process_content(&field.value, &mut drop_matches, &mut clog_matches, 
                                &mut drop_alt1_matches, &mut drop_alt2_matches, &mut drop_alt3_matches,
                                &mut clog_alt1_matches, &mut clog_alt2_matches, &mut clog_alt3_matches,
                                &mut unknown_formats, &mut value_matched);
                            message_matched |= value_matched;
                        }
                    }
                    
                    if let Some(footer) = &embed.footer {
                        info!("  Footer: {}", footer.text);
                    }
                }
            }
            
            // Process message content with regex patterns
            let mut content_matched = false;
            process_content(content, &mut drop_matches, &mut clog_matches, 
                &mut drop_alt1_matches, &mut drop_alt2_matches, &mut drop_alt3_matches,
                &mut clog_alt1_matches, &mut clog_alt2_matches, &mut clog_alt3_matches,
                &mut unknown_formats, &mut content_matched);
            message_matched |= content_matched;
            
            // If no part of the message matched any pattern, add content to unknown formats
            if !message_matched && !content.trim().is_empty() {
                unknown_formats.push(content.to_string());
            }
            
        } else if is_bot {
            other_bot_messages += 1;
            debug!("Other bot message: Author: {}, Content: {}", msg.author.name, content);
        } else {
            user_messages += 1;
            debug!("User message: Author: {}, Content: {}", msg.author.name, content);
        }
        
        // Skip non-RuneLite messages
        if author_id != RUNELITE_BOT_ID {
            continue;
        }
        
    }
    
    // Print message source statistics
    info!("Message source statistics:");
    info!("  - RuneLite messages: {}", runelite_messages);
    info!("  - User messages: {}", user_messages);
    info!("  - Other bot messages: {}", other_bot_messages);
    info!("  - Total messages: {}", runelite_messages + user_messages + other_bot_messages);
    
    // Print analysis results
    info!("Regex analysis results:");
    info!("  - Drop messages found (original): {}", drop_matches);
    info!("  - Drop messages found (alt1): {}", drop_alt1_matches);
    info!("  - Drop messages found (alt2): {}", drop_alt2_matches);
    info!("  - Drop messages found (alt3): {}", drop_alt3_matches);
    info!("  - Collection log entries found (original): {}", clog_matches);
    info!("  - Collection log entries found (alt1): {}", clog_alt1_matches);
    info!("  - Collection log entries found (alt2): {}", clog_alt2_matches);
    info!("  - Collection log entries found (alt3): {}", clog_alt3_matches);
    info!("  - Unknown RuneLite message formats: {}", unknown_formats.len());
    
    // Log all unknown formats for analysis
    for (i, format) in unknown_formats.iter().enumerate() {
        warn!("Unknown RuneLite format #{}: {}", i + 1, format);
    }
    
    Ok(())
}

// Helper function to process content with regex patterns
fn process_content(
    content: &str,
    drop_matches: &mut i32,
    clog_matches: &mut i32,
    drop_alt1_matches: &mut i32,
    drop_alt2_matches: &mut i32,
    drop_alt3_matches: &mut i32,
    clog_alt1_matches: &mut i32,
    clog_alt2_matches: &mut i32,
    clog_alt3_matches: &mut i32,
    unknown_formats: &mut Vec<String>,
    matched: &mut bool
) {
    // Track if this specific content matched any pattern
    let mut content_matched = false;
    
    // Try original DROP_REGEX
    if DROP_REGEX.is_match(content) {
        *drop_matches += 1;
        info!("DROP (original): {}", content);
        
        if let Some(captures) = DROP_REGEX.captures(content) {
            let player = captures.get(1).map_or("", |m| m.as_str()).trim();
            let item = captures.get(2).map_or("", |m| m.as_str()).trim();
            let quantity = captures.get(3).map_or("1", |m| m.as_str());
            let value = captures.get(4).map_or("0", |m| m.as_str());
            
            info!("  Player: {}", player);
            info!("  Item: {}", item);
            info!("  Quantity: {}", quantity);
            info!("  Value: {}", value);
        }
        *matched = true;
        content_matched = true;
    }
    
    // Try DROP_REGEX_ALT1
    if DROP_REGEX_ALT1.is_match(content) {
        *drop_alt1_matches += 1;
        info!("DROP (alt1): {}", content);
        
        if let Some(captures) = DROP_REGEX_ALT1.captures(content) {
            let player = captures.get(1).map_or("", |m| m.as_str()).trim();
            let item = captures.get(2).map_or("", |m| m.as_str()).trim();
            let quantity = captures.get(3).map_or("1", |m| m.as_str());
            let value = captures.get(4).map_or("0", |m| m.as_str());
            
            info!("  Player: {}", player);
            info!("  Item: {}", item);
            info!("  Quantity: {}", quantity);
            info!("  Value: {}", value);
        }
        *matched = true;
        content_matched = true;
    }
    
    // Try DROP_REGEX_ALT2
    if DROP_REGEX_ALT2.is_match(content) {
        *drop_alt2_matches += 1;
        info!("DROP (alt2): {}", content);
        
        if let Some(captures) = DROP_REGEX_ALT2.captures(content) {
            let player = captures.get(1).map_or("", |m| m.as_str()).trim();
            let item = captures.get(2).map_or("", |m| m.as_str()).trim();
            let quantity = captures.get(3).map_or("1", |m| m.as_str());
            let value = captures.get(4).map_or("0", |m| m.as_str());
            
            info!("  Player: {}", player);
            info!("  Item: {}", item);
            info!("  Quantity: {}", quantity);
            info!("  Value: {}", value);
        }
        *matched = true;
        content_matched = true;
    }
    
    // Try DROP_REGEX_ALT3
    if DROP_REGEX_ALT3.is_match(content) {
        *drop_alt3_matches += 1;
        info!("DROP (alt3): {}", content);
        
        if let Some(captures) = DROP_REGEX_ALT3.captures(content) {
            let player = captures.get(1).map_or("", |m| m.as_str()).trim();
            let item = captures.get(2).map_or("", |m| m.as_str()).trim();
            let quantity = captures.get(3).map_or("1", |m| m.as_str());
            let value = captures.get(4).map_or("0", |m| m.as_str());
            
            info!("  Player: {}", player);
            info!("  Item: {}", item);
            info!("  Quantity: {}", quantity);
            info!("  Value: {}", value);
        }
        *matched = true;
        content_matched = true;
    }
    
    // Try original CLOG_REGEX
    if CLOG_REGEX.is_match(content) {
        *clog_matches += 1;
        info!("CLOG (original): {}", content);
        
        if let Some(captures) = CLOG_REGEX.captures(content) {
            let player = captures.get(1).map_or("", |m| m.as_str()).trim();
            let item = captures.get(2).map_or("", |m| m.as_str()).trim();
            
            info!("  Player: {}", player);
            info!("  Item: {}", item);
        }
        *matched = true;
        content_matched = true;
    }
    
    // Try CLOG_REGEX_ALT1
    if CLOG_REGEX_ALT1.is_match(content) {
        *clog_alt1_matches += 1;
        info!("CLOG (alt1): {}", content);
        
        if let Some(captures) = CLOG_REGEX_ALT1.captures(content) {
            let player = captures.get(1).map_or("", |m| m.as_str()).trim();
            let item = captures.get(2).map_or("", |m| m.as_str()).trim();
            
            info!("  Player: {}", player);
            info!("  Item: {}", item);
        }
        *matched = true;
        content_matched = true;
    }
    
    // Try CLOG_REGEX_ALT2
    if CLOG_REGEX_ALT2.is_match(content) {
        *clog_alt2_matches += 1;
        info!("CLOG (alt2): {}", content);
        
        if let Some(captures) = CLOG_REGEX_ALT2.captures(content) {
            let player = captures.get(1).map_or("", |m| m.as_str()).trim();
            let item = captures.get(2).map_or("", |m| m.as_str()).trim();
            
            info!("  Player: {}", player);
            info!("  Item: {}", item);
        }
        *matched = true;
        content_matched = true;
    }
    
    // Try CLOG_REGEX_ALT3
    if CLOG_REGEX_ALT3.is_match(content) {
        *clog_alt3_matches += 1;
        info!("CLOG (alt3): {}", content);
        
        if let Some(captures) = CLOG_REGEX_ALT3.captures(content) {
            let player = captures.get(1).map_or("", |m| m.as_str()).trim();
            let item = captures.get(2).map_or("", |m| m.as_str()).trim();
            
            info!("  Player: {}", player);
            info!("  Item: {}", item);
        }
        *matched = true;
        content_matched = true;
    }
    
    // If content didn't match any pattern and isn't empty, add to unknown formats
    if !content_matched && !content.trim().is_empty() {
        unknown_formats.push(content.to_string());
    }
} 