use std::collections::HashMap;

use dashmap::DashMap;
use post_archiver::manager::PostArchiverManager;
use serde::{Deserialize, Serialize};

const PATREON_ARCHIVE_FEATURE: &str = "patreon-archive";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Context {
    pub campaigns: DashMap<String, CachedCampaign>,
}

impl Context {
    pub fn load(manager: &PostArchiverManager) -> Self {
        let (_, extra) = manager
            .get_feature_with_extra(PATREON_ARCHIVE_FEATURE)
            .unwrap_or_default();

        let json = serde_json::to_value(&extra).unwrap();
        serde_json::from_value(json).unwrap_or_default()
    }

    pub fn save(&self, manager: &PostArchiverManager) {
        let extras = HashMap::from([(
            "campaigns".to_string(),
            serde_json::to_value(&self.campaigns).unwrap(),
        )]);
        manager.set_feature_with_extra(PATREON_ARCHIVE_FEATURE, 1, extras);
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
