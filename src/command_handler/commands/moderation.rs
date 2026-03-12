use std::collections::HashMap;
use std::ops::Index;

use anyhow::Result;
use serenity::all::{
    CommandInteraction,
    CreateInteractionResponse,
    CreateInteractionResponseMessage,
    EditInteractionResponse,
};
use sqlx::SqlitePool;
use crate::command_handler::CollectionLogManagerKey;
use crate::rank_manager;
use crate::logger;
use crate::runescape_tracker::RunescapeTrackerKey;
use sqlx::{QueryBuilder, Row, Sqlite};

pub struct ItemData {
    item_id: i64,
    item_name: String,
    percentage: f64,
    clamp: bool,
    old_points: i64,
    points: i64,
    affected: i64,
}

#[derive(sqlx::FromRow)]
pub struct ClogRow {
    id: i64,
    discord_id: String,
    points: i64,
    item_name: String,
}

pub struct PlayerStats {
    change: i64,
    name: String,
}

pub async fn handle_recalculate( //Big red button
    command: &CommandInteraction,
    ctx: &serenity::prelude::Context,
    db: &SqlitePool,
) -> Result<()> {
    command
        .create_response(&ctx.http, CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new()
                .content("Recalculating...")
        ))
        .await?;
    let data = ctx.data.read().await;
    //This query assumes:
    //Item should have a non-zero amount of clogs for us to care about it
    //Clamps may have been removed or added, and we want to fix any problem clogs
    //Whitelists may have been removed or added, same reason
    //Only low completion percentage clogs are an issue, so we check all those (a clamp may have been removed instead of adding to whitelist)
    let item_records = sqlx::query!(
        //I have to list every column to remove type inferrence issues ughhhhhhhhh
        "SELECT item_id, item_name, preferred_name, categories, percentage, highest_points as 'highest_points!: i64', whitelist, clog_count, clamp, clamped_category from v_item_data
        WHERE clog_count > 0 AND ((clamp = 1 AND highest_points > 3000)
        OR whitelist = 1 OR percentage < 10)
        GROUP BY item_name ORDER BY percentage" //Until we work off item_id we gotta take care of dupes. Assume it's the most acquired one
    )
    .fetch_all(db)
    .await?;
    let mut running_count = 0;

    tracing::info!("Found {} relevant recalculation records", item_records.len());
    if item_records.len() > 0 {

        let mut item_vector: Vec<ItemData> = vec![];

        let clog_manager = data.get::<CollectionLogManagerKey>().unwrap();
        let rs_manager = data.get::<RunescapeTrackerKey>().unwrap();

        let mut clog_query: QueryBuilder<Sqlite> = QueryBuilder::new(
            "SELECT * FROM collection_log_entries WHERE item_name IN (",
        );

        let mut clog_query_separated = clog_query.separated(", ");
        for (i, record) in item_records.iter().enumerate() {

            clog_query_separated.push(format!("\"{}\"", record.item_name.clone().unwrap()));

            let old_points: i64 = record.highest_points;
            item_vector.push(ItemData {
                item_id: record.item_id,
                item_name: record.item_name.clone().unwrap(),
                percentage: record.percentage.clone().unwrap().parse::<f64>().unwrap(),
                clamp: if record.clamp == 1 && record.whitelist == Some(0) { true } else { false },
                old_points: old_points,
                points: clog_manager.calculate_points(record.item_name.clone().unwrap().as_str()).await.unwrap(),
                affected: 0,
            }); 
        }
        clog_query_separated.push_unseparated(")");

        let clog_records: Vec<ClogRow> = clog_query.build_query_as::<ClogRow>()
        .fetch_all(db)
        .await?;

        tracing::info!("Found {} relevant clog records", clog_records.len());

        if clog_records.len() > 0 {

            let mut affected_player_deltas: HashMap<String, i64> = HashMap::new();
            let mut pending_clog_updates: Vec<(i64, i64)> = Vec::new(); // (entry_id, new_points)

            

            for (i, row) in clog_records.into_iter().enumerate() {
                let target_item = item_vector.iter().position(|item| item.item_name == row.item_name).unwrap();

                let point_delta = item_vector[target_item].points - row.points; //Positive if new number bigger, negative otherwise

                let discord_id = row.discord_id;

                //Only lower points if we're clamping it. We don't want to lower points if we don't have to
                if ((point_delta.is_negative()) && item_vector[target_item].clamp) || ((point_delta.is_positive()) && !item_vector[target_item].clamp) {
                    running_count += 1;

                    tracing::info!("User {} point change from {}: {}", discord_id, item_vector[target_item].item_name, point_delta);

                    affected_player_deltas
                        .entry(discord_id.clone())
                        .and_modify(|player_delta| *player_delta += point_delta)
                        .or_insert(point_delta);

                    pending_clog_updates.push((row.id, item_vector[target_item].points));

                    item_vector[target_item].affected += 1;
                }
                
            }

            tracing::info!("Total edited record count: {}", running_count);

            if running_count > 0 {
                let mut tx = db.begin().await?;

                for (entry_id, new_points) in &pending_clog_updates {
                    sqlx::query("UPDATE collection_log_entries SET points = ? WHERE id = ?")
                        .bind(*new_points)
                        .bind(*entry_id)
                        .execute(&mut *tx)
                        .await?;
                }

                for (discord_id, point_delta) in &affected_player_deltas {
                    // Ensure user row exists before applying the delta.
                    sqlx::query(
                        "INSERT INTO users (discord_id, points, total_drops) VALUES (?, 0, 0)
                         ON CONFLICT(discord_id) DO NOTHING",
                    )
                    .bind(discord_id)
                    .execute(&mut *tx)
                    .await?;

                    sqlx::query("UPDATE users SET points = points + ? WHERE discord_id = ?")
                        .bind(*point_delta)
                        .bind(discord_id)
                        .execute(&mut *tx)
                        .await?;
                }

                tx.commit().await?;

                // Post-commit lookups keep the transaction short and avoid external/API latency in tx.
                let mut affected_players = HashMap::new();
                for (discord_id, point_delta) in affected_player_deltas.into_iter() {
                    let rs_name = rs_manager
                        .get_username_from_discord_id(ctx, discord_id.as_str())
                        .await
                        .unwrap_or_else(|_| "Unknown user".to_string());

                    let new_points = sqlx::query("SELECT points FROM users WHERE discord_id = ?")
                        .bind(discord_id.as_str())
                        .fetch_one(db)
                        .await
                        .map(|row| row.get::<i64, _>("points"))
                        .unwrap_or(0);
                    let old_points = new_points - point_delta;

                    if let Err(err) = rank_manager::notify_rank_transition(
                        ctx,
                        discord_id.as_str(),
                        &rs_name,
                        old_points,
                        new_points,
                        db,
                    )
                    .await
                    {
                        tracing::error!(
                            "Failed to emit rank transition notification for {}: {:?}",
                            discord_id,
                            err
                        );
                    }

                    affected_players.insert(
                        discord_id,
                        PlayerStats {
                            change: point_delta,
                            name: rs_name,
                        },
                    );
                }

                let mut info_readout = format!("\n**Recalculation Results** (only highest points previously awarded listed):\n{} total records affected!", running_count);

                for (i, data) in item_vector.into_iter().enumerate() {
                    if data.affected > 0 {
                        let point_delta = data.points - data.old_points;
                        info_readout += format!("\n**{}** ({}): from {} to {} points (**{}{}**), {} clogs affected",
                            data.item_name,
                            data.item_id,
                            data.old_points,
                            data.points,
                            if point_delta.is_positive() {"+"} else {""},
                            point_delta,
                            data.affected).as_str();
                    }
                    
                }

                info_readout += "\n**Affected users:**";

                for (i, player) in affected_players.into_iter().enumerate() {
                    info_readout += format!("\n**{}** ({}): **{}{}** points",
                    player.1.name,
                    player.0,
                    if player.1.change.is_positive() {"+"} else {""},
                    player.1.change,
                    ).as_str();
                }
                command
                    .edit_response(&ctx.http, EditInteractionResponse::new().content("Recalculation Complete!"))
                    .await?;
                let commanding_officer_id = command.user.id.to_string();
                logger::log_action(ctx, &commanding_officer_id, "recalculate", &info_readout).await?;
                
            }
        }
    }
    if running_count == 0 {
        command
            .edit_response(&ctx.http, EditInteractionResponse::new().content("Nothing to report, sheriff!"))
            .await?;
    }
    Ok(())
}

pub async fn handle_clamp(
    command: &CommandInteraction,
    ctx: &serenity::prelude::Context,
    db: &SqlitePool,
    on_or_off: bool,
) -> Result<()> {
    let options = &command.data.options;
    let category_name = options
        .iter()
        .find(|opt| opt.name == "category")
        .and_then(|opt| opt.value.as_str())
        .ok_or_else(|| anyhow::anyhow!("Category name not provided"))?;

    let one_or_zero = if on_or_off {1} else {0};

    sqlx::query!("UPDATE category_table SET clamp=? WHERE category=?", one_or_zero, category_name)
    .execute(db).await?;

    let response_string = format!("{} is now {}", category_name, if one_or_zero == 1 {
        "clamped! Items in this category will only give a maximum of 3000 points."
    } else {"unclamped! Items in this category can go beyond 3000 points!"});

    command
                .create_response(&ctx.http, CreateInteractionResponse::Message(
                    CreateInteractionResponseMessage::new()
                        .content(response_string)
                ))
                .await?;
    let commanding_officer_id = command.user.id.to_string();
    logger::log_action(ctx, &commanding_officer_id, if one_or_zero == 1 {"CLAMPED"} else {"UNCLAMPED"}, &category_name).await?;
    Ok(())
}

pub async fn handle_whitelist( //These are both basically the same function but I can't be assed to combine them rn
    command: &CommandInteraction,
    ctx: &serenity::prelude::Context,
    db: &SqlitePool,
    on_or_off: bool,
) -> Result<()> {
    let options = &command.data.options;
    let item_name = options
        .iter()
        .find(|opt| opt.name == "item")
        .and_then(|opt| opt.value.as_str())
        .ok_or_else(|| anyhow::anyhow!("Item name not provided"))?;

    let one_or_zero = if on_or_off {1} else {0}; //WHY DO I GET LIFETIME ERRORS UNLESS I DO IT LIKE THIS??
    //Even refreshing the bool as a bool in ANY way still gives the same error I'm so tired

    sqlx::query!("UPDATE collection_log_items SET whitelist=? WHERE item_name=?", one_or_zero, item_name)
    .execute(db).await?;

    let response_string = format!("{} is now {}", item_name, if one_or_zero == 1 {
        "whitelisted! Its points will never be clamped, even if it's in a clamped category."
    } else {"unwhitelisted! It's now subject to clamps!"});

    command
                .create_response(&ctx.http, CreateInteractionResponse::Message(
                    CreateInteractionResponseMessage::new()
                        .content(response_string)
                ))
                .await?;
    let commanding_officer_id = command.user.id.to_string();
    logger::log_action(ctx, &commanding_officer_id, if one_or_zero == 1 {"WHITELISTED"} else {"UNWHITELISTED"}, &item_name).await?;
    Ok(())
}

// async fn is_allowed( //I wrote this and then found out you can specify required perms
//     command: &CommandInteraction,
//     ctx: &serenity::prelude::Context,
// ) -> Result<bool> {
//     let config = Config::from_env()?;
//     let admin_role = config.admin_role_id;
//     let allowed = command.member.unwrap().roles.contains(&admin_role.unwrap());

//     if !allowed {
//         command
//                 .create_response(&ctx.http, CreateInteractionResponse::Message(
//                     CreateInteractionResponseMessage::new()
//                         .content("Sorry kitten, you're not allowed to do that.")
//                 ))
//                 .await?;
//     }

//     Ok(allowed)
// }