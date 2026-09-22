use axum::Extension;
use serenity::all::ArgumentConvert;
use serenity::all::CreateEmbedAuthor;
use serenity::all::CreateEmbedFooter;
use serenity::all::{
    CreateAttachment, Timestamp
};
use serenity::prelude::*;
use serenity::all::{CreateMessage, CreateEmbed, Member, GuildId, ChannelId};
use sqlx::SqlitePool;
use axum::{
    body::Bytes,
    extract::{Request, Multipart, FromRequest, Path},
    http::{header::CONTENT_TYPE, StatusCode},
    response::{IntoResponse, Response},
};
use tracing::{error, debug};
use crate::DinkHandler;
use serde::Deserialize;
use crate::logger;
use crate::rank_manager;
use crate::command_handler::utils;

// https://github.com/pajlads/DinkPlugin/blob/master/docs/json-examples.md


#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
struct DinkItem {
    id: i64,
    quantity: i64,
    price_each: i64,
    name: String,
    criteria: Vec<String>,
    rarity: Option<f64>,
}
// All of these need to be Options because they may or may not be there
#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
struct DinkExtra {
    // Loot category
    source: Option<String>,
    items: Option<Vec<DinkItem>>,
    category: Option<String>,
    kill_count: Option<i32>,
    // Pet category
    pet_name: Option<String>,
    milestone: Option<String>,
    duplicate: Option<bool>,
    // Clog category
    item_name: Option<String>,
    item_id: Option<i64>,
    price: Option<i64>,
    completed_entries: Option<i32>,
    total_entries: Option<i32>,
    dropper_name: Option<String>,
    dropper_type: Option<String>,
    dropper_kill_count: Option<i32>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
struct DinkDiscord {
    id: String,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
struct DinkPayload {
    // These are always sent
    content: Option<String>,
    #[serde(alias = "type")]
    notif_type: String,
    player_name: String,
    account_type: String,
    seasonal_world: bool,
    dink_account_hash: String,
    discord_user: Option<DinkDiscord>,
    extra: DinkExtra,
}

struct DinkFile {
    file_name: String,
    content: Bytes,
}

// identify_user looks a runescape_accounts row up two different ways (by hash, then by name)
// that need to agree on a row type - naming it explicitly here is what lets query_as! do that,
// since each query! call site otherwise gets its own anonymous struct type.
struct LinkedAccount {
    id: i64,
    discord_id: String,
    runescape_name: String,
    dink_hash: Option<String>,
}

//Struct to check for the collection log in two different ways
struct CollectionLogItem {
    item_id: i64,
    item_name: String,
    preferred_name: String,
    percentage: String,
}

pub async fn dink_handler(Extension(handler): Extension<DinkHandler>, Path(token): Path<String>, req: Request) -> Response {
    // The token is the only thing gating this endpoint - Dink can't send custom headers, so it
    // has to live in the URL path itself. Distribute it only via the hosted, importable Dink
    // config, not by hand. 404 (not 401) so the endpoint's existence isn't confirmed either way.
    if token != handler.config.dink_webhook_token {
        return StatusCode::NOT_FOUND.into_response();
    }

    let Some(agent_str) = req.headers()
        .get("user-agent")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
    else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    // This wouldn't scale well but these are the only two we need to worry about
    // Check user-agent to minimize bad requests
    if !(agent_str.starts_with("RuneLite/") || agent_str.starts_with("HDOS/")) {
        return StatusCode::FORBIDDEN.into_response();
    }

    let content_type = req.headers()
        .get(CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();

    // Dink only sends multipart/form-data when a screenshot is attached; if there's none, it
    // POSTs a plain application/json body instead. Both need to be accepted, or every
    // screenshot-less notification gets rejected before it ever reaches the logic below.
    let (payload_json, dink_file): (Bytes, Option<DinkFile>) = if content_type.starts_with("multipart/form-data") {
        let mut multipart = match Multipart::from_request(req, &()).await {
            Ok(m) => m,
            Err(_) => return StatusCode::BAD_REQUEST.into_response(),
        };

        let mut payload_json: Option<Bytes> = None;
        let mut dink_file: Option<DinkFile> = None;

        loop {
            let field = match multipart.next_field().await {
                Ok(Some(field)) => field,
                Ok(None) => break,
                Err(_) => return StatusCode::BAD_REQUEST.into_response(),
            };
            let field_name = field.name().unwrap_or("").to_string();
            match field_name.as_str() {
                "payload_json" => {
                    debug!("Found payload");
                    payload_json = field.bytes().await.ok();
                }
                "file" => {
                    debug!("Found screenshot");
                    let file_name = field.file_name().unwrap_or("screenshot.png").to_string();
                    if let Ok(content) = field.bytes().await {
                        dink_file = Some(DinkFile { file_name, content });
                    }
                }
                other => error!("Error handling field: {:?}", other),
            }
        }

        let Some(payload_json) = payload_json else {
            return StatusCode::BAD_REQUEST.into_response();
        };
        (payload_json, dink_file)
    } else if content_type.starts_with("application/json") {
        match Bytes::from_request(req, &()).await {
            Ok(bytes) => (bytes, None),
            Err(_) => return StatusCode::BAD_REQUEST.into_response(),
        }
    } else {
        return StatusCode::UNSUPPORTED_MEDIA_TYPE.into_response();
    };

    debug!("Payload length: {}", payload_json.len());
    let data: DinkPayload = match serde_json::from_slice(&payload_json) {
        Ok(data) => data,
        Err(e) => {
            error!("Failed to parse Dink payload: {:?}", e);
            return StatusCode::BAD_REQUEST.into_response();
        }
    };

    process_dink_event(handler, data, dink_file).await;
    StatusCode::OK.into_response()
}

async fn process_dink_event(dink_handler: DinkHandler, data: DinkPayload, dink_file: Option<DinkFile>) {
    let config = &dink_handler.config;
    let screenshot = dink_file.map(|f| CreateAttachment::bytes(f.content, f.file_name));
    let mut author = CreateEmbedAuthor::new(data.player_name.clone());
        if data.account_type.as_str() != "NORMAL" { //Mains don't have rights
            let icon = match data.account_type.as_str() {
                "IRONMAN" => "Ironman_chat_badge.png",
                "ULTIMATE_IRONMAN" => "Ultimate_ironman_chat_badge.png",
                "HARDCORE_IRONMAN" => "Hardcore_ironman_chat_badge.png",
                "GROUP_IRONMAN" => "Group_ironman_chat_badge.png",
                "HARDCORE_GROUP_IRONMAN" => "Hardcore_group_ironman_chat_badge.png",
                "UNRANKED_GROUP_IRONMAN" => "Unranked_group_ironman_chat_badge.png",
                _ => "Cheese_detail.png",
            };
            author = author.icon_url(format!("https://oldschool.runescape.wiki/images/{icon}"));
        }
        let mut embed = CreateEmbed::new()
        .author(author)
        .timestamp(Timestamp::now());
        if let Some(ref shot) = screenshot {
            embed = embed.image(format!("attachment://{}", shot.filename));
        }
        let Some(member) = identify_user(data.clone(), dink_handler.db.clone(), dink_handler.ctx.clone(), dink_handler.guild_id, config.runelite_channel_id).await else {
            //If we can't find the user an account belongs to, they probably shouldn't be getting posted
            let _ = logger::log_generic(
                &dink_handler.ctx,
                &format!("UNKNOWN USER: Someone I couldn't find in the discord server tried to do something: RSN {} sent something with Dink of type {}", data.player_name, data.notif_type)
            ).await;
            return;
        };
        if data.seasonal_world {
            //Don't care about leagues (for now)
            debug!("Ignoring Dink event from seasonal/league world for {}", data.player_name);
            return;
        }
        {
            let footer = CreateEmbedFooter::new(member.display_name())
            .icon_url(member.face());
            embed = embed.footer(footer);

            let discord_id = member.user.id.to_string();

            let mut sendable = false;

            match data.notif_type.as_str() {
                "COLLECTION" => {
                    sendable = true;
                    debug!("Received collection log");
                    let Some(id) = data.extra.item_id else {
                        debug!("COLLECTION event with no itemId, dropping");
                        return;
                    };
                    let Some(name) = data.extra.item_name else {
                        debug!("COLLECTION event with no item name, dropping");
                        return;
                    };
                    let item = identify_item(id.clone(), name.clone(), dink_handler.db.clone()).await;
                    //Initiate
                    let description: String;
                    if let Some(item) = item {
                        let item_name = item.preferred_name.clone();
                        //Use our own item_id from here on, in case the one we got was unreliable
                        //(Meaning we did a name lookup instead)
                        let item_id = item.item_id.clone();
                        //Do they have this item recorded already?
                        if let Ok(Some(_)) = sqlx::query!(
                            "SELECT id FROM collection_log_entries 
                            WHERE discord_id = ? AND item_name = ?",
                            discord_id,
                            item_name
                        )
                        .fetch_optional(&dink_handler.db)
                        .await
                        {
                            debug!("User {} already has collection log entry for {}", discord_id, item_name);
                            description = format!("Got a collection log item:\n**{}**!\n\n...But it was already recorded!", search_link(item_name.clone()));

                            let _ = logger::log_action(
                                &dink_handler.ctx,
                                &discord_id,
                                "DINK CLOG",
                                &format!("{} received collection log item they already had: {}", data.player_name, item_name)
                            ).await;
                        } else {
                            let (points, new_total) = dink_clog(&dink_handler, item_id, item_name.clone(), discord_id.clone(), &member.display_name()).await;
                            description = format!("Got a new collection log item:\n**{}**!", search_link(item_name.clone()));
                            //Now that we know for sure the item is valid we can build the embed

                            let percentage = item.percentage.clone();
                            embed = embed.field("Global Clog Rate", format_value(format!("{}%", percentage)), true)
                            .field("Collection Log", format_value(format!("{}/{}",
                                data.extra.completed_entries.unwrap_or_default(),
                                data.extra.total_entries.unwrap_or_default())), true)
                            .field("", "", false);

                            if data.extra.dropper_name.is_some() || data.extra.dropper_kill_count.is_some() {
                                embed = field_if_exists(embed, data.extra.dropper_name, "Source");
                                embed = field_if_exists_int(embed, data.extra.dropper_kill_count, "Count");
                                embed = embed.field("", "", false);
                            }

                            embed = embed.field("Points Added", format_value(format!("+{}", points)), true)
                            .field("Points Total", format_value(new_total.to_string()), true)
                            .thumbnail(format!("https://static.runelite.net/cache/item/icon/{item_id}.png"));
                            
                            let _ = logger::log_action(
                                &dink_handler.ctx,
                                &discord_id,
                                "DINK CLOG",
                                &format!("{} received collection log item: {} (+{} points)", data.player_name, item_name, points)
                            ).await;
                        }
                        
                    }
                    else {
                        //If we don't have a record for it, it's probably new
                        //We don't have data so we kinda just have to abandon ship
                        description = format!("Got a new collection log item:\n**{}**!\n\nBut, ummm... I don't know what that is yet... sorry...", search_link(name.clone()));
                        //We can still add the record but no points will be added
                        dink_clog(&dink_handler, id, name.clone(), discord_id.clone(), &member.display_name()).await;
                        let _ = logger::log_action(
                                &dink_handler.ctx,
                                &discord_id,
                                "DINK CLOG",
                                &format!("{} received collection log item, but it was unknown (zero points given): {}", data.player_name, name)
                            ).await;
                        embed = embed.thumbnail(format!("https://static.runelite.net/cache/item/icon/{id}.png"));
                    }
                    embed = embed.description(description);
                }
                "LOOT" => {
                    debug!("Received drop");
                    //Currently, drops worth 100k+ gp = 1 point per 100k
                    //We first need to check if the drop is even worth that much
                    //Part of the reason for this update is because RDT uncut sapphires (for example) are technically ~1/170
                    //Technically *rare* when some valuable drops are 1/128, but when people had their drop plugins set improperly we'd get notified for basically everything
                    //Dink does not have rarity set for every drop (according to docs), so we'll just say if it's high enough value it's fine
                    let Some(items) = data.extra.items else {
                        debug!("LOOT event with no items, dropping");
                        return;
                    };
                    let mut valuable: Option<DinkItem> = None;
                    let mut best: i64 = 0;
                    for (_i, item) in items.iter().enumerate() {
                        //The number might be low depending on the users RuneLite settings (shop sell price instead of GE price), so get more reliable numbers.
                        //But if we don't have a live GE price for this item at all (unwrap_or(0)),
                        //fall back to what Dink itself reported instead of treating it as worthless -
                        //a missing/stale price shouldn't silently suppress an otherwise-valuable drop.
                        let price = item.price_each.max(dink_handler.price_manager.get_item_id_price(&item.id).await.unwrap_or(0));
                        let value = item.quantity * price;
                        //Annoyingly even if an item is in the denylist, it's still sent if we get other drop data, just with DENYLIST criteria
                        if value > best && !item.criteria.contains(&"DENYLIST".to_string()) {
                            valuable = Some(item.clone());
                            best = value;
                        }
                    }
                    //Only count it if it meets our value threshold
                    if best >= 100_000 {
                        let item = valuable.unwrap();
                        //Now that we know it's valuable, we're okay to send
                        sendable = true;
                        let points = best / 100_000;
                        let source = data.extra.source.clone().unwrap_or_else(|| "an unknown source".to_string());
                        let description = if item.quantity > 1 {
                            format!("Got {}x {} from {}!", item.quantity, search_link(item.name.clone()), search_link(source))
                        }
                        else {
                            format!("Got {} from {}!", search_link(item.name.clone()), search_link(source))
                        };
                        if let Some(rarity_val) = item.rarity {
                            let denom = (1.0 / rarity_val).round();
                            let rarity = rarity_val * 100.0;
                            embed = embed.field("Rarity (approx)", format!("```glsl\n# 1/{} ({:.2}%)```", denom, rarity), true);
                        }
                        let new_total = dink_drop(&dink_handler, item.id, item.name.clone(), best, discord_id.clone(), &member.display_name()).await;
                        embed = embed.description(description)
                        .field("GE Price", format_value(utils::format_gp(best)), true)
                        .field("", "", false)
                        .field("Points Added", format_value(format!("+{}", points)), true)
                        .field("Points Total", format_value(new_total.to_string()), true)
                        .thumbnail(format!("https://static.runelite.net/cache/item/icon/{}.png", item.id));

                        // Log the auto-added drop to the bot log channel
                        let _ = crate::logger::log_action(
                            &dink_handler.ctx,
                            &discord_id,
                            "DINK DROP",
                            &format!("{} received {}x {} worth {} GP", data.player_name, item.quantity, item.name, best)
                        ).await;
                    }
                    else {
                        if let Some(item) = valuable {
                            let _ = crate::logger::log_action(
                                &dink_handler.ctx,
                                &discord_id,
                                "DINK DROP (REJECTED)",
                                &format!("{} tried to log some hot garbage: Most valuable item was {}x {} worth {} GP", data.player_name, item.quantity, item.name, best)
                            ).await;
                        }
                    }
                }
                "PET" => {
                    sendable = true;
                    debug!("Received pet");
                    //We don't process this for points (yet) so it's pretty simple, relatively
                    let description = if data.extra.duplicate.unwrap_or(false) {
                        "You have a funny feeling like you would've been followed..."
                    } else {
                        "You have a funny feeling like you're being followed..."
                    };
                    embed = embed.description(description);
                    if let Some(pet_name) = data.extra.pet_name.clone() { //Is the name set?
                        embed = embed.field("Pet", format_value(pet_name.clone()), true);
                        //Check db for item_id (Dink doesn't give this for pets)
                        let pet_row = sqlx::query!(
                            "SELECT item_id, item_name FROM collection_log_items WHERE item_name = ?",
                            pet_name
                        )
                        .fetch_one(&dink_handler.db)
                        .await;
                        if let Ok(pet_row) = pet_row {
                            let pet_id = pet_row.item_id;
                            embed = embed.thumbnail(format!("https://static.runelite.net/cache/item/icon/{pet_id}.png"));
                        }
                        if let Some(milestone) = data.extra.milestone.clone() { //Do we have a milestone?
                            embed = embed.field("Milestone", format_value(milestone), true);
                        }
                    }
                }
                _ => {
                    debug!("Received type we don't handle");
                    let _ = logger::log_action(
                        &dink_handler.ctx,
                        &member.user.id.to_string(),
                        "UNSUPPORTED EVENT",
                        &format!("{} tried to use Dink type {}", data.player_name, data.notif_type)
                    ).await;
                }
            }
            if sendable {
                let builder = CreateMessage::new().add_embed(embed);
                let Some(channel_id) = config.runelite_channel_id else {
                    error!("RUNELITE_CHANNEL_ID not configured, dropping Dink notification");
                    return;
                };
                let send_result = match screenshot {
                    Some(shot) => channel_id.send_files(&dink_handler.ctx.http, [shot], builder).await.map(|_| ()),
                    None => channel_id.send_message(&dink_handler.ctx.http, builder).await.map(|_| ()),
                };
                if let Err(why) = send_result {
                    error!("Failed to send Dink notification: {:?}", why);
                }
            }
        }
}
//This function is so if you want to change the formatting on everything, you can ("fix" makes the text blue)
fn format_value (value: String) -> String {
    format!("```fix\n{value}```")
}
async fn identify_user (data: DinkPayload, db: SqlitePool, ctx: Context, guild_id: GuildId, channel_id: Option<ChannelId>) -> Option<Member> {
    let mut member: Option<Member> = None;

    //Okay who are we dealing with here
    //dink_hash is stable across in-game renames (that's the whole reason it exists - see
    //https://github.com/sariyamelody/kittyscape-loot-bot/issues/10), so check that first: it finds
    //a renamed player's existing row without needing anything else. Only fall back to matching by
    //name, and only onto a row with no hash on file yet - matching *any* row by name here would let
    //a new player who claims an old, renamed-away-from name silently take over that old account
    //(the bug fixed in c17cd22), which this still needs to avoid.
    let username = data.player_name;
    let hash = data.dink_account_hash;
    //dink_account_hash is a required field, but a player who's never run ::dinkhash may still
    //send "" rather than omitting it. Treat that the same as "no hash on file" everywhere - if we
    //matched on dink_hash = '' directly, every unconfigured player would collide with every other
    //unconfigured player the moment either of them got a "" written to their row below.
    let hash_opt: Option<String> = (!hash.is_empty()).then(|| hash.clone());
    let user = match &hash_opt {
        Some(h) => match sqlx::query_as!(
            LinkedAccount,
            "SELECT id, discord_id, runescape_name, dink_hash FROM runescape_accounts WHERE dink_hash = ?",
            h
        )
        .fetch_one(&db)
        .await
        {
            found @ Ok(_) => found,
            Err(_) => sqlx::query_as!(
                LinkedAccount,
                "SELECT id, discord_id, runescape_name, dink_hash FROM runescape_accounts WHERE runescape_name = ? AND dink_hash IS NULL",
                username
            )
            .fetch_one(&db)
            .await,
        },
        None => sqlx::query_as!(
            LinkedAccount,
            "SELECT id, discord_id, runescape_name, dink_hash FROM runescape_accounts WHERE runescape_name = ? AND dink_hash IS NULL",
            username
        )
        .fetch_one(&db)
        .await,
    };
    //Did we find anyone
    if let Ok(user) = user {
        //Ok cool. Does all our info match?
        if !(user.dink_hash == hash_opt && user.runescape_name == username) {
            //Either we don't have a hash yet, or the username got changed
            let _ = sqlx::query!("UPDATE runescape_accounts
            SET runescape_name = ?, dink_hash = ?
            WHERE id = ?",
            username, hash_opt, user.id)
            .execute(&db).await;
        }
        let member_data = Member::convert(ctx, Some(guild_id), channel_id, &user.discord_id.as_str()).await;
        if let Ok(member_found) = member_data {
            member = Some(member_found);
        }
    }
    //Well who the fuck is this then
    //Do they have discord data in the notification?
    else if let Some(discord_user) = data.discord_user {
        //Is this person actually in our server?
        let discord_id = discord_user.id;
        let member_data = Member::convert(ctx.clone(), Some(guild_id), channel_id, discord_id.as_str()).await;
        if let Ok(member_found) = member_data {
            //If they're in the server, and using the plugin, we assume they WANT to be tracked
            //We'll just help them automagically
            member = Some(member_found);

            //If the user is totally new, make sure we have a row for them
            let _ = sqlx::query!(
                "INSERT INTO users (discord_id, points, total_drops)
                VALUES (?, 0, 0)
                ON CONFLICT(discord_id) DO NOTHING",
                discord_id
            )
            .execute(&db)
            .await;

            // Link RS name to Discord ID
            let _ = sqlx::query!(
                "INSERT INTO runescape_accounts (discord_id, runescape_name, dink_hash)
                VALUES (?, ?, ?)
                ON CONFLICT(discord_id, runescape_name) DO NOTHING",
                discord_id,
                username,
                hash_opt
            )
            .execute(&db)
            .await;

            // Log the action
            let _ = logger::log_action(
                &ctx,
                &discord_id,
                "AUTOLINKED RSNAME",
                &format!("{}", username)
            ).await;
        }
    }
    member
}
/// Records a Dink collection log entry and awards points (if any) through
/// `rank_manager::add_points`, instead of a raw `UPDATE users`. That gets us rank-up/down
/// notifications firing the same way they do for the /clog command, and the caller can show the
/// real post-event points total instead of whatever was on hand before this event.
/// collection_log_entries.discord_id has a foreign key on users.discord_id (enforced - sqlx
/// enables PRAGMA foreign_keys by default), so this relies on the `users` row already existing by
/// the time it's called - `identify_user` guarantees that: either it matched an existing
/// runescape_accounts row (which, by the same FK, couldn't exist without a users row) or it just
/// upserted one itself on the auto-link path.
/// Returns (points awarded, user's new points total).
async fn dink_clog(handler: &DinkHandler, item_id: i64, name: String, discord_id: String, user_name: &str) -> (i64, i64) {

    let points = handler.collection_log_manager.calculate_points_dink(item_id).await.unwrap_or(0);

    // Record the collection log entry
    let _ = sqlx::query!(
        "INSERT INTO collection_log_entries (discord_id, item_name, points, item_id) VALUES (?, ?, ?, ?)",
        discord_id,
        name,
        points,
        item_id
    )
    .execute(&handler.db)
    .await;

    match rank_manager::add_points(&handler.ctx, &discord_id, user_name, points, &handler.db).await {
        Ok(update) => (points, update.new_points),
        Err(e) => {
            error!("Failed to record points for Dink clog: {:?}", e);
            (points, 0)
        }
    }
}
/// Records a Dink drop and increments total_drops (which the previous raw-SQL version never
/// touched, unlike the /drop command - /stats and /leaderboard both read it directly), then
/// awards points through `rank_manager::add_points` (see dink_clog for why).
/// Returns the user's new points total.
async fn dink_drop(handler: &DinkHandler, item_id: i64, name: String, value: i64, discord_id: String, user_name: &str) -> i64 {

    // Record the drop
    let _ = sqlx::query!(
        "INSERT INTO drops (discord_id, item_name, value, item_id) VALUES (?, ?, ?, ?)",
        discord_id,
        name,
        value,
        item_id
    )
    .execute(&handler.db)
    .await;

    let points = value / 100_000;
    match rank_manager::add_points(&handler.ctx, &discord_id, user_name, points, &handler.db).await {
        Ok(update) => {
            let _ = sqlx::query!(
                "UPDATE users
                SET total_drops = total_drops + 1
                WHERE discord_id = ?",
                discord_id
            )
            .execute(&handler.db)
            .await;
            update.new_points
        },
        Err(e) => {
            error!("Failed to record points for Dink drop: {:?}", e);
            0
        }
    }
}
fn field_if_exists(embed: CreateEmbed, value: Option<String>, name: &str) -> CreateEmbed {
    if let Some(value) = value { embed.field(name, format!("```\n{value}```"), true) } else { embed }
}
fn field_if_exists_int(embed: CreateEmbed, value: Option<i32>, name: &str) -> CreateEmbed {
    if let Some(value) = value { embed.field(name, format!("```\n{}```", value.to_string()), true) } else { embed }
}
fn search_link(name: String) -> String {
    let link = format!("https://oldschool.runescape.wiki/w/Special:Search?search={}", name.clone().replace(" ", "%20"));
    format!("[{}]({})", name, link)
}
//Identify a collection log item
//If we cannot find the item by id (meaning Dink likely gave us a bad id, or we don't have it yet)
//Then we use the name as a fallback
async fn identify_item(id: i64, name: String, db: SqlitePool) -> Option<CollectionLogItem> {
    match sqlx::query_as!(
            CollectionLogItem,
            "SELECT item_id, item_name, preferred_name, percentage FROM v_item_data WHERE item_id = ?",
            id
        )
        .fetch_one(&db)
        .await
        .ok()
        {
            found @ Some(_) => found,
            None => sqlx::query_as!(
                CollectionLogItem,
                "SELECT item_id, item_name, preferred_name, percentage FROM v_item_data WHERE item_name = ?",
                name
            )
            .fetch_one(&db)
            .await
            .ok(),
        }
}