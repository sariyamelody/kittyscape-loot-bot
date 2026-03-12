use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info, debug};

const USER_AGENT: &str = "KittyScape Loot Bot/1.0";

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ItemMapping {
    pub examine: Option<String>,
    pub id: i64,
    pub members: Option<bool>,
    #[serde(rename = "lowalch")]
    pub low_alch: Option<i64>,
    pub limit: Option<i64>,
    pub value: Option<i64>,
    #[serde(rename = "highalch")]
    pub high_alch: Option<i64>,
    pub icon: Option<String>,
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LatestPrices {
    pub data: HashMap<String, ItemPrice>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ItemPrice {
    pub high: Option<i64>,
    pub high_time: Option<i64>,
    pub low: Option<i64>,
    pub low_time: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct PriceData {
    pub mappings: HashMap<String, ItemMapping>,
    pub latest_prices: HashMap<i64, ItemPrice>,
}

pub struct PriceManager {
    data: Arc<RwLock<PriceData>>,
    client: reqwest::Client,
}

impl PriceManager {
    pub async fn new() -> Result<Self> {
        let client = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .build()?;

        let mappings = Self::fetch_mappings(&client).await?;
        info!("PriceManager initialized with {} items", mappings.len());
        
        // Debug log some example items
        for (name, mapping) in mappings.iter().take(5) {
            debug!("Example price item: {} (ID: {})", name, mapping.id);
        }

        let data = PriceData {
            mappings,
            latest_prices: HashMap::new(),
        };

        let manager = Self {
            data: Arc::new(RwLock::new(data)),
            client,
        };

        // Do initial price update
        manager.update_prices().await?;

        Ok(manager)
    }

    async fn fetch_mappings(client: &reqwest::Client) -> Result<HashMap<String, ItemMapping>> {
        let response = client
            .get("https://prices.runescape.wiki/api/v1/osrs/mapping")
            .send()
            .await?
            .json::<Vec<ItemMapping>>()
            .await?;

        let mut mappings = HashMap::new();
        for item in response {
            mappings.insert(item.name.clone(), item);
        }

        info!("Loaded {} items from mapping", mappings.len());
        Ok(mappings)
    }

    pub async fn update_prices(&self) -> Result<()> {
        let response = self.client
            .get("https://prices.runescape.wiki/api/v1/osrs/latest")
            .send()
            .await?
            .json::<LatestPrices>()
            .await?;

        let mut data = self.data.write().await;
        data.latest_prices.clear();

        for (id_str, price) in response.data {
            if let Ok(id) = id_str.parse::<i64>() {
                data.latest_prices.insert(id, price);
            }
        }

        info!("Updated prices for {} items", data.latest_prices.len());
        Ok(())
    }

    pub async fn start_price_updates(self: Arc<Self>) {
        tokio::spawn(async move {
            loop {
                if let Err(e) = self.update_prices().await {
                    error!("Failed to update prices: {}", e);
                }
                tokio::time::sleep(tokio::time::Duration::from_secs(600)).await;
            }
        });
    }

    pub async fn get_item_suggestions(&self, partial: &str) -> Vec<String> {
        let data = self.data.read().await;
        let partial = partial.to_lowercase();
        
        data.mappings
            .keys()
            .filter(|name| name.to_lowercase().contains(&partial))
            .take(25)  // Discord has a limit of 25 choices
            .cloned()
            .collect()
    }

    pub async fn get_item_price(&self, name: &str) -> Option<i64> {
        let data = self.data.read().await;
        
        // Find the item mapping
        let mapping = data.mappings.get(name)?;
        
        // Get the latest price
        let price = data.latest_prices.get(&mapping.id)?;
        
        // Use the lowest available price, defaulting to high alch value if available, or 0 if not
        Some(price.low
            .or(price.high)
            .or(mapping.high_alch)
            .unwrap_or(0))
    }
} 