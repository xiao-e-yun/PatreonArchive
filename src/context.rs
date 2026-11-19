use std::{
    collections::{HashMap, HashSet},
    fs::read_to_string,
    path::Path,
};

use log::warn;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Context {
    /// campaign_id: CachedCampaign
    pub campaigns: HashMap<String, CachedCampaign>,
}

impl Context {
    pub const RELATION_PATH: &'static str = "configs/patreon-archive.json";

    pub fn load(path: &Path) -> Self {
        let path = path.join(Self::RELATION_PATH);
        let Ok(json) = read_to_string(&path) else {
            return Self::default();
        };
        serde_json::from_str(&json).unwrap_or_else(|e| {
            warn!("Failed to parse {}, starting fresh: {e}", path.display());
            Self::default()
        })
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
    /// Post IDs that failed to import; retried on every run.
    #[serde(default)]
    pub failed: HashSet<String>,
}

impl CachedCampaign {
    /// Only trust `published` while the pledge hasn't gone up: a higher pledge may unlock
    /// older posts, so the whole campaign has to be rescanned.
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
