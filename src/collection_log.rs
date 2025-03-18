use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, debug, error};
use serde_json::Value;
use html_escape::decode_html_entities;

const USER_AGENT: &str = "KittyScape Loot Bot/1.0";
const WIKI_API_URL: &str = "https://oldschool.runescape.wiki/api.php";

#[derive(Debug, Clone)]
pub struct CollectionLogData {
    pub completion_rates: HashMap<String, f64>,
}

pub struct CollectionLogManager {
    data: Arc<RwLock<CollectionLogData>>,
    client: reqwest::Client,
}

impl CollectionLogManager {
    pub async fn new() -> Result<Self> {
        let client = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .build()?;

        let completion_rates = Self::fetch_completion_rates(&client).await?;
        info!("CollectionLogManager initialized with {} items", completion_rates.len());
        
        // Debug log some example items
        for (name, rate) in completion_rates.iter().take(5) {
            debug!("Example collection log item: {} - {}%", name, rate);
        }

        let data = CollectionLogData {
            completion_rates,
        };

        Ok(Self {
            data: Arc::new(RwLock::new(data)),
            client,
        })
    }

    async fn fetch_completion_rates(client: &reqwest::Client) -> Result<HashMap<String, f64>> {
        let mut rates = HashMap::new();
        
        let params = [
            ("action", "parse"),
            ("page", "Collection_log/Table"),
            ("format", "json"),
            ("prop", "text"),
        ];

        info!("Fetching collection log data from wiki API...");
        let response = client
            .get(WIKI_API_URL)
            .query(&params)
            .send()
            .await?;
        
        info!("Got response with status: {}", response.status());
        let response_text = response.text().await?;
        debug!("Response text length: {} bytes", response_text.len());
        
        let json: Value = serde_json::from_str(&response_text)?;
        
        if let Some(html) = json.get("parse")
            .and_then(|p| p.get("text"))
            .and_then(|t| t.get("*"))
            .and_then(|s| s.as_str()) 
        {
            debug!("Successfully got HTML content from response");
            debug!("HTML content length: {} bytes", html.len());
            
            let document = scraper::Html::parse_document(html);
            let selector = scraper::Selector::parse("tr").unwrap();
            let rows: Vec<_> = document.select(&selector).collect();
            debug!("Found {} table rows", rows.len());

            for (i, row) in rows.iter().enumerate() {
                debug!("Processing row {}", i);
                
                // Log the raw HTML of the row for debugging
                debug!("Row HTML: {}", row.html());
                
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
                    debug!("Found item name: {}", name);
                    
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
                        if !name.is_empty() {
                            debug!("Found item: {} with rate: {}%", name, rate);
                            rates.insert(name, rate);
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

        // Fallback to some default items if we failed to parse any
        if rates.is_empty() {
            error!("Failed to parse any items from wiki, using fallback items");
            rates.insert("Twisted bow".to_string(), 0.2);
            rates.insert("Dragon Warhammer".to_string(), 0.5);
        }

        info!("Initialized collection log with {} items", rates.len());
        Ok(rates)
    }

    pub async fn calculate_points(&self, item_name: &str) -> Option<i64> {
        let data = self.data.read().await;
        let completion_rate = data.completion_rates.get(item_name)?;

        // Multi-tiered point calculation
        let points = if *completion_rate <= 5.0 {
            // Tier 3: Mega-rare items (≤5%)
            // 5% -> 500 points
            // 3% -> 1000 points
            // 1% -> 15000 points
            // 0.5% -> 30000 points
            let base = 100.0;
            let rarity_multiplier = (1.0 / completion_rate).powf(1.5) * 30.0;
            base * rarity_multiplier
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
} 