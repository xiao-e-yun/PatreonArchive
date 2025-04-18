pub mod comment;
pub mod post;

use std::sync::Arc;

use jsonapi_deserialize::JsonApiDeserialize;

#[derive(Debug, Clone, JsonApiDeserialize)]
#[json_api(rename_all = "snake_case")]
pub struct User {
    pub id: String,
    pub full_name: String,
}

#[derive(Debug, Clone, JsonApiDeserialize)]
#[json_api(rename_all = "snake_case")]
pub struct Member {
    #[allow(unused)]
    pub id: String,
    pub campaign_currency: String,
    pub campaign_pledge_amount_cents: Option<u32>,
    #[json_api(relationship = "single", resource = "Campaign")]
    pub campaign: Arc<Campaign>,
}

impl Member {
    pub fn cents(&self) -> u32 {
        self.campaign_pledge_amount_cents.unwrap_or_default()
    }
}

#[derive(Debug, Clone, JsonApiDeserialize)]
#[json_api(rename_all = "snake_case")]
pub struct Campaign {
    pub id: String,
    #[allow(unused)]
    pub is_active: bool,
    pub name: String,
    pub url: String,
}
