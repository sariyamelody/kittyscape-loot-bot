use anyhow::Result;
use serenity::prelude::*;
use serenity::model::id::UserId;
use crate::config::ConfigKey;

/// Logs an action to the bot's log channel
pub async fn log_action(
    ctx: &Context,
    user_id: &str,
    action_type: &str,
    details: &str,
) -> Result<()> {
    let data = ctx.data.read().await;
    if let Some(config) = data.get::<ConfigKey>() {
        // Try to get the user's display name
        let user_name = if let Ok(user_id_num) = user_id.parse::<u64>() {
            let user_id_obj = UserId::new(user_id_num);
            match ctx.http.get_user(user_id_obj).await {
                Ok(user) => format!("{} ({})", user.name, user_id),
                Err(_) => user_id.to_string()
            }
        } else {
            user_id.to_string()
        };
        
        // Format log message
        let log_message = format!(
            "**[{}]** `{}` {}: {}",
            chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC"),
            user_name,
            action_type,
            details
        );
        
        // Send to log channel
        if let Err(why) = config.log_channel_id.say(&ctx.http, log_message).await {
            tracing::error!("Failed to send log message: {:?}", why);
        }
    }
    
    Ok(())
} 
//Log other messages that might not follow the normal formatting
pub async fn log_generic(
    ctx: &Context,
    details: &str,
) -> Result<()> {
    let data = ctx.data.read().await;
    if let Some(config) = data.get::<ConfigKey>() {

        // Format log message
        let log_message = format!(
            "**[{}]** {}",
            chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC"),
            details
        );

        // Send to log channel
        if let Err(why) = config.log_channel_id.say(&ctx.http, log_message).await {
            tracing::error!("Failed to send log message: {:?}", why);
        }
    }
    Ok(())
}