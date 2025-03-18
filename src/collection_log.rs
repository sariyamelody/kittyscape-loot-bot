use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, debug};

const USER_AGENT: &str = "KittyScape Loot Bot/1.0";

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

        let completion_rates = Self::initialize_completion_rates();
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

    fn initialize_completion_rates() -> HashMap<String, f64> {
        let mut rates = HashMap::new();
        
        // Boss drops (very rare)
        rates.insert("Twisted Bow".to_string(), 0.1);
        rates.insert("Scythe of vitur".to_string(), 0.15);
        rates.insert("Elysian sigil".to_string(), 0.05);
        rates.insert("Dragon Warhammer".to_string(), 0.5);
        rates.insert("Tumeken's shadow".to_string(), 0.1);
        
        // Rare boss drops
        rates.insert("Bandos chestplate".to_string(), 1.0);
        rates.insert("Armadyl crossbow".to_string(), 1.2);
        rates.insert("Zamorakian spear".to_string(), 1.5);
        rates.insert("Ancestral robe top".to_string(), 1.0);
        rates.insert("Kodai insignia".to_string(), 0.8);
        
        // Medium rarity items
        rates.insert("Dragon boots".to_string(), 5.0);
        rates.insert("Abyssal whip".to_string(), 8.0);
        rates.insert("Berserker ring".to_string(), 4.0);
        rates.insert("Dragon chainbody".to_string(), 6.0);
        rates.insert("Staff of the dead".to_string(), 3.0);
        
        // Common items
        rates.insert("Rune platebody".to_string(), 25.0);
        rates.insert("Dragon med helm".to_string(), 15.0);
        rates.insert("Dragon dagger".to_string(), 20.0);
        rates.insert("Mystic robe top".to_string(), 30.0);
        rates.insert("Dragon longsword".to_string(), 18.0);

        // Raid items
        rates.insert("Dexterous prayer scroll".to_string(), 2.0);
        rates.insert("Arcane prayer scroll".to_string(), 2.0);
        rates.insert("Dragon claws".to_string(), 0.8);
        rates.insert("Ancestral hat".to_string(), 1.0);
        rates.insert("Dinh's bulwark".to_string(), 1.2);

        // Wilderness items
        rates.insert("Dragon pickaxe".to_string(), 3.0);
        rates.insert("Ring of the gods".to_string(), 0.5);
        rates.insert("Tyrannical ring".to_string(), 1.0);
        rates.insert("Treasonous ring".to_string(), 1.0);
        rates.insert("Odium ward".to_string(), 1.5);

        // Slayer items
        rates.insert("Abyssal dagger".to_string(), 2.0);
        rates.insert("Kraken tentacle".to_string(), 4.0);
        rates.insert("Occult necklace".to_string(), 5.0);
        rates.insert("Imbued heart".to_string(), 0.3);
        rates.insert("Primordial crystal".to_string(), 1.0);

        info!("Initialized collection log with {} items", rates.len());
        rates
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