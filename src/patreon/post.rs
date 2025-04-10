use std::{collections::HashMap, sync::Arc};

use jsonapi_deserialize::JsonApiDeserialize;
use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Clone, JsonApiDeserialize)]
#[json_api(rename_all = "snake_case")]
pub struct Post {
    pub id: String,
    pub comment_count: u32,
    pub current_user_can_view: bool,
    pub image: Option<Image>,
    pub min_cents_pledged_to_view: Option<u32>,
    /// UNKNOWN
    #[json_api(default)]
    pub embed: Option<Embed>,
    #[json_api(default)]
    pub content: Option<String>,
    #[json_api(default)]
    pub post_metadata: Option<PostMetadata>,
    pub post_type: String,
    pub published_at: String,
    pub title: String,
    pub url: String,
    // #[json_api(relationship = "optional", resource = "Vec<Media>")]
    // pub audio: Option<Arc<Vec<Media>>>,
    // #[json_api(relationship = "optional", resource = "Vec<Media>")]
    // pub images: Option<Arc<Vec<Media>>>,
}

impl Post {
    pub fn required_cents(&self) -> u32 {
        self.min_cents_pledged_to_view.unwrap_or_default()
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Image {
    pub large_url: String,
    pub thumb_square_large_url: String,
    pub thumb_square_url: String,
    pub thumb_url: String,
    pub url: String,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Embed {
    description: Option<String>,
    html: Option<String>,
    linked_object_id: Option<String>,
    linked_object_type: Option<String>,
    product_variant_id: Option<String>,
    provider: Option<String>,
    provider_url: Option<String>,
    subject: Option<String>,
    url: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct PostMetadata {
    #[serde(default)]
    pub image_order: Vec<String>,
}

#[derive(Debug, Clone, JsonApiDeserialize)]
#[json_api(rename_all = "snake_case")]
pub struct Media {
    pub id: String,
    pub file_name: String,
    pub download_url: Option<String>,
    pub image_urls: Option<MediaImageUrls>,
    pub metadata: MediaMetadata,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MediaImageUrls {
    pub default: String,
    pub default_blurred: String,
    pub default_blurred_small: String,
    pub default_large: String,
    pub default_small: String,
    pub original: String,
    pub thumbnail: String,
    pub thumbnail_large: String,
    pub thumbnail_small: String,
    pub url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MediaMetadata {
    pub dimensions: Option<MediaMetadataDimensions>,
    #[serde(flatten)]
    pub others: HashMap<String, Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MediaMetadataDimensions {
    pub w: u32,
    pub h: u32,
}
