#![allow(unused)]

use std::{collections::HashMap, sync::Arc};

use jsonapi_deserialize::JsonApiDeserialize;
use post_archiver::importer::{UnsyncContent, UnsyncFileMeta};
use serde::Deserialize;
use serde_json::Value;

use crate::post::file::PatreonFileMeta;

#[derive(Debug, Clone, JsonApiDeserialize)]
#[json_api(rename_all = "snake_case")]
pub struct Post {
    pub id: String,
    pub comment_count: u32,
    pub current_user_can_view: bool,
    pub image: Option<Image>,
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
    #[json_api(relationship = "optional", resource = "Media")]
    pub audio: Option<Arc<Media>>,
    #[json_api(relationship = "optional", resource = "Media")]
    pub audio_preview: Option<Arc<Media>>,
    #[json_api(relationship = "multiple", resource = "Media")]
    pub media: Vec<Arc<Media>>,
    #[json_api(relationship = "optional", resource = "Poll")]
    pub poll: Option<Arc<Poll>>,
    #[json_api(relationship = "multiple", resource = "ContentUnlockOption")]
    pub content_unlock_options: Vec<Arc<ContentUnlockOption>>,
    #[json_api(relationship = "multiple", resource = "PostTag")]
    pub user_defined_tags: Vec<Arc<PostTag>>,
}

impl Post {
    pub fn is_free(&self) -> bool {
        self.content_unlock_options.is_empty()
            || self
                .content_unlock_options
                .iter()
                .any(|e| e.reward.patron_amount_cents == 0)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Image {
    pub large_url: String,
    pub thumb_square_large_url: String,
    pub thumb_square_url: String,
    pub thumb_url: String,
    pub url: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Embed {
    description: Option<String>,
    html: Option<String>,
    linked_object_id: Option<String>,
    linked_object_type: Option<String>,
    product_variant_id: Option<u32>,
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
    pub file_name: Option<String>,
    pub download_url: String,
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
    pub duration_s: Option<u32>,
    #[serde(flatten)]
    pub others: HashMap<String, Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MediaMetadataDimensions {
    pub w: u32,
    pub h: u32,
}

#[derive(Debug, Clone, JsonApiDeserialize)]
pub struct Poll {
    #[json_api(relationship = "multiple", resource = "PollChoice")]
    pub choices: Vec<Arc<PollChoice>>,
}

#[derive(Debug, Clone, JsonApiDeserialize)]
#[json_api(rename_all = "snake_case")]
pub struct PollChoice {
    pub position: u32,
    pub num_responses: u32,
    pub text_content: String,
}

#[derive(Debug, Clone, JsonApiDeserialize)]
#[json_api(resource_type = "content-unlock-option")]
pub struct ContentUnlockOption {
    pub id: String,
    #[json_api(relationship = "single", resource = "Reward")]
    pub reward: Arc<Reward>,
}

#[derive(Debug, Clone, JsonApiDeserialize)]
#[json_api(rename_all = "snake_case")]
pub struct Reward {
    pub id: String,
    pub patron_amount_cents: u32,
}

#[derive(Debug, Clone, JsonApiDeserialize)]
#[json_api(rename_all = "snake_case")]
pub struct PostTag {
    pub id: String,
    pub value: String,
}
