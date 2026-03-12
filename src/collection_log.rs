use anyhow::Result;
use scraper::ElementRef;
use serenity::futures::{AsyncReadExt, TryStreamExt};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info};
use serde_json::Value;
use html_escape::decode_html_entities;
use sqlx::{QueryBuilder, Row, Sqlite, SqlitePool, query};

const USER_AGENT: &str = "KittyScape Loot Bot/1.0";
const WIKI_API_URL: &str = "https://oldschool.runescape.wiki/api.php";

#[derive(Debug, Clone)]
pub struct CollectionLogData {
    pub completion_rates: HashMap<String, f64>,
}

pub struct CollectionLogItem {
    pub item_id: f64,
    pub item_name: String,
    pub preferred_name: String,
    pub percentage: f64,
    pub categories: String,
    //pub release_date: String,
}


pub struct CollectionLogManager<> {
    data: Arc<RwLock<CollectionLogData>>,
    db: SqlitePool,
}

impl CollectionLogManager<> {
    pub async fn new(db: &SqlitePool) -> Result<Self> {
        let client = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .build()?;

        let completion_items: u64 = Self::fetch_completion_rates(&client, &db).await?;
        info!("CollectionLogManager initialized with {} items", completion_items);

        let completion_data = sqlx::query!(
            "SELECT item_name, percentage FROM collection_log_items",
        )
        .fetch_all(db)
        .await?;

        let mut completion_rates: HashMap<String, f64> = HashMap::new();

        for comp_data_item in completion_data.iter() {
            let item_name = comp_data_item.item_name.clone();
            let percentage = comp_data_item.percentage.clone();
            completion_rates.insert(item_name.unwrap(), percentage.unwrap().parse::<f64>().unwrap());
        }

        // Debug log some example items
        for (name, rate) in completion_rates.iter().take(5) {
            debug!("Example collection log item: {} - {}%", name, rate);
        }

        let data = CollectionLogData {
            completion_rates,
        };

        Ok(Self {
            data: Arc::new(RwLock::new(data)),
            db: db.clone(),
        })
    }

    async fn fetch_completion_rates(client: &reqwest::Client, db: &SqlitePool) -> Result<u64> {
        let mut items: Vec<CollectionLogItem> = Vec::new();
        
        
        let table_params = [
            ("action", "parse"),
            ("page", "Collection_log/Table"),
            ("format", "json"),
            ("prop", "text"),
        ];
        
        //I wanted to fetch dates but that might be overcomplicating things. I'm leaving the param code here at least
        // let bucket_params = [
        //     ("action", "bucket"),
        //     ("query", "bucket('infobox_item').select('item_id','item_name','release_date').where({'Category:Collection log items'}).offset().run()"),
        //     ("format", "json"),
        // ];

        info!("Fetching collection log data from wiki API...");
        let response = client
            .get(WIKI_API_URL)
            .query(&table_params)
            .send()
            .await?;
        
        info!("Got response with status: {}", response.status());
        let response_text = response.text().await?;
        debug!("Response text length: {} bytes", response_text.len());
        
        let json: Value = serde_json::from_str(&response_text)?;

        let mut data_insert: QueryBuilder<Sqlite> = QueryBuilder::new(
            "INSERT INTO collection_log_items (item_id, item_name, preferred_name, percentage, categories) VALUES "
        );

        let mut data_insert_separated = data_insert.separated(", ");
        
        if let Some(html) = json.get("parse")
            .and_then(|p| p.get("text"))
            .and_then(|t| t.get("*"))
            .and_then(|s| s.as_str()) 
        {
            debug!("Successfully got HTML content from response");
            debug!("HTML content length: {} bytes", html.len());
            
            
            //The data we're parsing, if you make it readable, looks like this:
            // <tr data-item-id="6571">
            // <td>
            // <span class="mw-default-size" typeof="mw:File">
            // <a href="/w/File:Uncut_onyx.png" class="mw-file-description">
            // <img src="/images/Uncut_onyx.png?ad4b1" decoding="async" loading="lazy" width="21" height="22" class="mw-file-element" data-file-width="21" data-file-height="22" />
            // </a>
            // </span>
            // <a href="/w/Uncut_onyx" title="Uncut onyx">
            // Uncut onyx</a>
            // </td>
            // <td>
            // <a href="/w/Fortis_Colosseum" title="Fortis Colosseum">
            // Fortis Colosseum</a>
            // , <a href="/w/Skotizo" title="Skotizo">
            // Skotizo</a>
            // , <a href="/w/Zalcano" title="Zalcano">
            // Zalcano</a>
            // , <a href="/w/Zulrah" title="Zulrah">
            // Zulrah</a>
            // , Miscellaneous</td>
            // <td class="table-bg-yellow">
            // 17.9%</td>
            // </tr>
            let document = scraper::Html::parse_document(html);
            let selector = scraper::Selector::parse("tr[data-item-id]").unwrap();
            let rows: Vec<_> = document.select(&selector).collect();
            debug!("Found {} table rows", rows.len());

            for (i, row) in rows.iter().enumerate() {
                debug!("Processing row {}", i);

                info!("{:#?}", row.value());
                let item_id = row.value().attr("data-item-id").unwrap().parse::<f64>().unwrap();
                
                // Log the raw HTML of the row for debugging
                info!("Row HTML: {}", row.html());
                
                let cells: Vec<_> = row.select(&scraper::Selector::parse("td").unwrap()).collect();
                debug!("Found {} cells in row", cells.len());
                
                if let Some(first_cell) = cells.first() {
                    debug!("First cell HTML: {}", first_cell.html());
                    
                    let links: Vec<_> = first_cell.select(&scraper::Selector::parse("a").unwrap()).collect();
                    debug!("Found {} links in first cell", links.len());
                    
                    if let Some(first_link) = links.first() {
                        debug!("First link HTML: {}", first_link.html());
                        debug!("First link attributes: {:?}", first_link.value().attrs);
                    }
                }
                
                if let Some(name) = row
                    .select(&scraper::Selector::parse("td").unwrap())
                    .next()
                    .and_then(|td| {
                        // Get all links in the cell
                        let links: Vec<_> = td.select(&scraper::Selector::parse("a").unwrap()).collect();
                        // Skip the image link (first link) and get the item name link (second link)
                        links.get(1)
                            .and_then(|a| a.value().attr("title"))
                            .map(|s| decode_html_entities(s).into_owned())
                    })
                {
                    info!("Found item name: {}", name);

                    let preferred_name = row
                    .select(&scraper::Selector::parse("td").unwrap())
                    .next().unwrap()
                    .select(&scraper::Selector::parse("a").unwrap())
                    .collect::<Vec<ElementRef>>()
                    .get(1).unwrap()
                    .text()
                    .collect::<String>();
                    
                    if let Some(rate) = row
                        .select(&scraper::Selector::parse("td").unwrap())
                        .last()
                        .and_then(|td| td.text().next())
                        .map(|s| s.trim())
                        .and_then(|s| {
                            debug!("Found rate text: {}", s);
                            if s.starts_with("<") {
                                // Handle rates shown as "<0.1%"
                                Some(0.1)
                            } else {
                                // Normal rate like "12.4%"
                                s.trim_end_matches('%')
                                    .parse::<f64>()
                                    .ok()
                            }
                        })
                    {
                        if let Some(categories) = Some(row
                            .select(&scraper::Selector::parse("td").unwrap())
                            .nth(1)
                            .unwrap()
                            .text()
                            .collect::<String>())
                            {
                                if !name.is_empty() {
                                    debug!("Found item: {} with rate: {}% and categories: {}", name, rate, categories);
                                    let item = CollectionLogItem{item_id: item_id,
                                    percentage: rate,
                                    categories: if name.contains("3rd age") {
                                        categories + ", Third Age"
                                    } else if name.contains("Gilded") {
                                        categories + ", Gilded"
                                    } else {
                                        categories
                                    },
                                    item_name: name,
                                    preferred_name: preferred_name};
                                    data_insert_separated.push(format_args!("(\"{}\", \"{}\", \"{}\", \"{}\", \"{}\")", item.item_id, item.item_name, item.preferred_name, item.percentage, item.categories));
                                    items.push(item);
                                }
                            }
                            else {
                                info!("Failed to parse categories for item: {}", name);
                            }
                    } else {
                        info!("Failed to parse rate for item: {}", name);
                    }
                } else {
                    info!("Failed to find item name in row");
                }
            }
        } else {
            error!("Failed to get HTML content from response");
            info!("Response JSON structure: {}", serde_json::to_string_pretty(&json)?);
        }

        data_insert.push("ON CONFLICT(item_id) DO UPDATE SET item_name=excluded.item_name, preferred_name=excluded.preferred_name, percentage=excluded.percentage, categories=excluded.categories");

        // let please_god = data_insert.into_sql();
        // info!("{}", please_god);
        data_insert.build().execute(db).await?;

        // v_categories_clogs is a recursive(!) table view that doubles as a sort of linking table. It's not pretty but it does exactly what I need it to do.
        // the recursion is necessary to split the "category" field into as many substrings as needed. The alternative is collecting all categories into a Vec<str> which Rust hates.
        sqlx::query!(
            "INSERT INTO category_table (category) SELECT category FROM v_categories_clogs GROUP BY category ON CONFLICT(category_table.category) DO NOTHING;")
        .execute(db)
        .await?;

        info!("Initialized collection log with {} items", items.len());
        Ok(items.len().try_into().unwrap())
    }

    pub async fn calculate_points(&self, item_name: &str) -> Option<i64> {
        let data = self.data.read().await;
        let completion_rate = data.completion_rates.get(item_name)?;
        let item_record = sqlx::query!(
            "SELECT * FROM v_item_data WHERE item_name LIKE '%' || ? || '%' ORDER BY item_id",
            item_name
        )
        .fetch_one(&self.db)
        .await
        .ok()?;
        
        // Multi-tiered point calculation
        let points = if *completion_rate <= 5.0 {
            // Tier 3: Mega-rare items (≤5%)
            // 5% -> 500 points
            // 3% -> 1000 points
            // 1% -> 15000 points
            // 0.5% -> 30000 points
            let base = 100.0;
            let rarity_multiplier = (1.0 / completion_rate).powf(1.5) * 30.0;
            //Is the item in a clamped category, and not whitelisted?
            //Only checked here because the other percentage categories are nowhere near 3k
            if item_record.whitelist == Some(0) && item_record.clamp > 0 {
                (base * rarity_multiplier).clamp(0.0, 3000.0)
            }
            else {
                base * rarity_multiplier
            }
        } else if *completion_rate <= 20.0 {
            // Tier 2: Moderately rare items (5-20%)
            // Linear interpolation between:
            // 20% -> 200 points
            // 5% -> 500 points
            let progress = (20.0 - completion_rate) / 15.0; // 0 to 1 scale
            200.0 + (progress * 300.0)
        } else {
            // Tier 1: Common items (>20%)
            // Simple linear scaling
            100.0 - (completion_rate * 0.5)
        };

        Some(points.round() as i64)
    }

    pub async fn get_suggestions(&self, partial: &str) -> Vec<String> {
        let data = self.data.read().await;
        let partial = partial.to_lowercase();

        data.completion_rates
            .keys()
            .filter(|name| name.to_lowercase().contains(&partial))
            .take(25)
            .cloned()
            .collect()
    }

    pub async fn get_category_suggestions(&self, partial: &str) -> Vec<String> {
        let partial = partial.to_lowercase();

        let mut query_suggestions = vec![];

        let query_results = sqlx::query!("SELECT category FROM category_table WHERE category LIKE '%' || ? || '%' LIMIT 25", partial)
        .fetch_all(&self.db)
        .await;

        for (i, result) in query_results.unwrap().into_iter().enumerate() {
            query_suggestions.push(result.category.unwrap());
        }

        query_suggestions
    }
} 