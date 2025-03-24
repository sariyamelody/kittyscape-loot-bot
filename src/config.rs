use std::env;
use serenity::model::id::ChannelId;
use serenity::prelude::TypeMapKey;

pub struct Config {
    pub mod_channel_id: ChannelId,
}

impl Config {
    pub fn from_env() -> Result<Self, env::VarError> {
        let mod_channel_id = env::var("MOD_CHANNEL_ID")?
            .parse::<u64>()
            .map_err(|_| env::VarError::NotPresent)?;

        Ok(Self {
            mod_channel_id: ChannelId::new(mod_channel_id),
        })
    }
}

pub struct ConfigKey;

impl TypeMapKey for ConfigKey {
    type Value = Config;
} 