use std::{fs::read_to_string, path::Path};

use dashmap::DashMap;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Context {
    pub campaigns: DashMap<String, CachedCampaign>,
}

impl Context {
    pub const RELATION_PATH: &'static str = "configs/patreon-archive.json";

    pub fn load(path: &Path) -> Self {
        let path = path.join(Self::RELATION_PATH);
        let json = read_to_string(path).unwrap_or_default();
        serde_json::from_str(&json).unwrap_or_default()
    }

    pub fn save(&self, path: &Path) {
        let path = path.join(Self::RELATION_PATH);
        std::fs::create_dir_all(path.parent().unwrap()).expect("Failed to create context folder");
        let json = serde_json::to_string(self).expect("Failed to serialize context");
        std::fs::write(path, json).expect("Failed to save context");
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CachedCampaign {
    pub published: i64,
    pub cents: u32,
}

impl CachedCampaign {
    pub fn last_published(&self, cents: u32) -> Option<i64> {
        let cents_unchanged = cents <= self.cents;
        cents_unchanged.then_some(self.published)
    }

    pub const fn update(&mut self, published: i64, cents: u32) {
        if published > self.published {
            self.published = published;
        }
        if cents > self.cents {
            self.cents = cents;
        }
    }
}
